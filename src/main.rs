//! Prometheus exporter for Pixiv illustration metrics.
//!
//! This binary can validate or generate configuration files, and run an HTTP
//! server exposing scraped illustration metrics at `/metrics`.

#![deny(clippy::unwrap_used)]

use std::{
    collections::HashSet, future::ready, net::IpAddr, path::PathBuf, sync::Arc, time::Duration,
};

use axum::{Router, routing::get};
use chrono::Local;
use clap::{Parser, Subcommand};
use futures_util::{StreamExt, pin_mut};
use log::{debug, error, info, warn};
use metrics_exporter_prometheus::PrometheusBuilder;
use pixiv3_rs::AppPixivAPI;
use soft_cycle::SoftCycleController;

mod config;
mod export;
mod logger;
mod unwrap_or_exit;

use crate::config::Config;
use crate::unwrap_or_exit::UnwrapOrExit;

/// The main command line interface.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The command to execute.
#[derive(Subcommand)]
enum Command {
    /// Check config, view config schema, or generate default config
    Config {
        #[command(subcommand)]
        config_command: ConfigCommand,
    },
    /// Start the exporter server, with the given config file
    Serve { config_path: PathBuf },
}

/// The subcommand for the config command.
#[derive(Subcommand)]
enum ConfigCommand {
    /// Print the JSON schema for the config file
    Schema,
    /// Print the default config
    Default,
    /// Check the config file
    Check { config_path: PathBuf },
}

/// Coordinates scraping work and the HTTP metrics service lifecycle.
struct Server {
    api: AppPixivAPI,
    config: Config,
    cycle_ctrl: Arc<SoftCycleController>,
}

impl Server {
    fn gen_independent_item_interval(&self) -> Duration {
        self.config.scrape.independent_item_interval.gen_interval()
    }

    fn gen_user_item_interval(&self) -> Duration {
        self.config.scrape.user_item_interval.gen_interval()
    }

    fn gen_scrape_interval(&self) -> Duration {
        self.config.scrape.scrape_interval.gen_interval()
    }
}

impl Server {
    /// Creates a server with the loaded runtime configuration and Pixiv token.
    fn new(config: Config, refresh_token: String) -> Self {
        Self {
            cycle_ctrl: Arc::new(SoftCycleController::new()),
            config,
            api: AppPixivAPI::new_from_refresh_token(refresh_token),
        }
    }

    /// Waits for either a duration to pass or a shutdown signal.
    ///
    /// Returns `Some(())` when the wait completes normally, and `None` when the
    /// cycle controller receives a shutdown notification.
    async fn waiting_with_cycle_ctrl(
        self: &Arc<Self>,
        desc: &str,
        duration: Duration,
    ) -> Option<()> {
        tokio::select! {
            _ = tokio::time::sleep(duration) => {
                Some(())
            }
            _ = self.cycle_ctrl.listener() => {
                warn!("Shutdown signal received while {desc}, exiting...");
                None
            }
        }
    }

    /// Runs the periodic scraper loop for configured users and works.
    ///
    /// Returns `Some(())` only if the loop exits naturally; returns `None` once
    /// a shutdown signal interrupts any waiting point.
    async fn illusts_scraper(self: Arc<Self>) -> Option<()> {
        info!("Starting illusts scraper...");
        let mut round = 0;

        loop {
            info!("Begin illusts scraping round #{}", round);

            let time_begin = Local::now();
            let mut visited = HashSet::new();

            for &user in &self.config.target.users {
                debug!("Scraping user #{user}");
                let illust_iter = self.api.user_illusts_iter(user, None, None, None, true);

                pin_mut!(illust_iter);
                while let Some(illust) = illust_iter.next().await {
                    let illust = match illust {
                        Ok(i) => i,
                        Err(e) => {
                            warn!("Failed to fetch illust from user #{user}: {e}, skip the user");
                            break;
                        }
                    };

                    if visited.insert(illust.id) {
                        export::export_illust_info(&illust);
                    }

                    self.waiting_with_cycle_ctrl(
                        "scraping user illusts",
                        self.gen_user_item_interval(),
                    )
                    .await?;
                }
            }

            for &work in &self.config.target.works {
                if visited.insert(work) {
                    debug!("Scraping work #{work}");
                    let illust = match self.api.illust_detail(work, true).await {
                        Ok(r) => r,
                        Err(e) => {
                            warn!(
                                "Failed to fetch illust detail for work #{work}: {e}, skip the work"
                            );
                            continue;
                        }
                    };

                    export::export_illust_info(&illust.illust);

                    self.waiting_with_cycle_ctrl(
                        "scraping illusts",
                        self.gen_independent_item_interval(),
                    )
                    .await?;
                }
            }

            let interval = self.gen_scrape_interval();
            let time_end = Local::now();
            let duration = time_end.signed_duration_since(time_begin).as_seconds_f64();
            let est_next_start = time_end + interval;
            info!(
                "End illusts scraping round #{round} in {duration:.3} seconds, {items} items scraped",
                items = visited.len()
            );
            info!(
                "Next round will start in {interval:.3} seconds, est. at {est_next_start}",
                interval = interval.as_secs_f64(),
            );

            round += 1;

            self.waiting_with_cycle_ctrl("waiting for next scraping round", interval)
                .await?;
        }
    }

    /// Starts all runtime tasks, installs metrics, and serves `/metrics`.
    ///
    /// This method launches the Ctrl+C handler, scraper loop, Prometheus upkeep,
    /// and Axum HTTP server, then waits for all tasks to finish.
    async fn serve(self: Arc<Self>) {
        let ctrl_c_handler_task = tokio::spawn({
            let cycle_ctrl = self.cycle_ctrl.clone();
            async move {
                info!("Ctrl+C handler started");
                tokio::signal::ctrl_c()
                    .await
                    .unwrap_or_exit_with(|e| error!("Failed to install Ctrl+C handler: {e}"));
                warn!("Ctrl+C received, shutting down...");
                let _ = cycle_ctrl.try_notify(());
                warn!("Shutdown signal sent, Ctrl+C handler exiting");
            }
        });
        let illusts_scraper_task = tokio::spawn(self.clone().illusts_scraper());

        let recorder_handle = PrometheusBuilder::new()
            .install_recorder()
            .unwrap_or_exit_with(|e| error!("Failed to install Prometheus metrics recorder: {e}"));
        export::describe_metrics();

        tokio::spawn({
            let recorder_handle = recorder_handle.clone();
            async move {
                const REPORT_INTERVAL: u64 = 200;

                info!("Prometheus recorder upkeeping task started");
                for tick in 1u64.. {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    recorder_handle.run_upkeep();

                    if tick % REPORT_INTERVAL == 0 {
                        info!("Prometheus recorder upkeeping task tick #{tick}");
                    }
                }
            }
        });

        let app = Router::new().route("/metrics", get(move || ready(recorder_handle.render())));

        let bind_addr = self
            .config
            .server
            .bind
            .parse::<IpAddr>()
            .unwrap_or_exit_with(|e| {
                error!("Invalid server.bind address in config: {e}; expected a valid IP address")
            });
        let bind_port = self.config.server.port;

        info!("Listening on {bind_addr}:{bind_port}");

        let axum_server = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind((bind_addr, bind_port))
                .await
                .unwrap_or_exit_with(|e| {
                    error!("Failed to bind TCP listener for HTTP server: {e}")
                });

            info!("Starting axum server...");
            axum::serve(listener, app)
                .with_graceful_shutdown({
                    let cycle_ctrl = self.cycle_ctrl.clone();
                    async move {
                        let _ = cycle_ctrl.listener().await;
                        warn!("Notifying a graceful shutdown to the axum server")
                    }
                })
                .await
        });

        let _ = tokio::join!(ctrl_c_handler_task, illusts_scraper_task, axum_server,);
    }
}

fn main() {
    match Cli::parse().command {
        Command::Config {
            config_command: ConfigCommand::Schema,
        } => {
            println!("{}", Config::json_schema());
        }
        Command::Config {
            config_command: ConfigCommand::Default,
        } => {
            println!("{}", Config::example_config());
        }
        Command::Config {
            config_command: ConfigCommand::Check { config_path },
        } => match Config::load_from_file(config_path) {
            Ok(config) => {
                println!("Config loaded successfully: {:?}", config);
            }
            Err(e) => {
                eprintln!("Failed to load config: {}", e);
                std::process::exit(1);
            }
        },
        Command::Serve { config_path } => {
            logger::init_logger();

            let config = Config::load_from_file(config_path)
                .unwrap_or_exit_with(|e| error!("Failed to load config: {e}"));

            debug!("Config loaded: {:?}", config);

            let refresh_token = config
                .refresh_token
                .get_value()
                .unwrap_or_exit_with(|e| error!("Failed to get refresh token: {e}"));

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_exit_with(|e| error!("Failed to build tokio runtime: {e}"))
                .block_on(Arc::new(Server::new(config, refresh_token)).serve());
        }
    }
}
