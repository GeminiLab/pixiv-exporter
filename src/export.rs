//! Prometheus metric registration and export for Pixiv illustrations.
//!
//! This module maps `IllustrationInfo` fields into gauge metrics with stable
//! labels that can be scraped from `/metrics`.

use log::debug;
use metrics::{Unit, describe_gauge, gauge};
use pixiv3_rs::models::IllustrationInfo;

use chrono::Utc;

/// Registers all gauge metrics emitted by the exporter.
pub fn describe_metrics() {
    describe_gauge!(
        "pixiv_illust_views",
        Unit::Count,
        "Total views of a illustration."
    );
    describe_gauge!(
        "pixiv_illust_bookmarks",
        Unit::Count,
        "Total bookmarks of a illustration."
    );
    describe_gauge!(
        "pixiv_illust_comments",
        Unit::Count,
        "Total comments of a illustration."
    );
    describe_gauge!(
        "pixiv_illust_uptime",
        Unit::Seconds,
        "Seconds since the illustration was created."
    );
    describe_gauge!(
        "pixiv_illust_page_count",
        Unit::Count,
        "Total pages of a illustration."
    );
    describe_gauge!("pixiv_illust_tag", "A tag of a illustration.");
    describe_gauge!("pixiv_illust_info", "Information of a illustration.")
}

/// Exports one illustration payload into Prometheus gauges.
///
/// Core counters are emitted with short labels (`id`, `user_id`), while
/// metadata is emitted via `pixiv_illust_info` with richer descriptive labels.
pub fn export_illust_info(illust: &IllustrationInfo) {
    debug!(
        "Exporting illust #{} from user #{}",
        illust.id, illust.user.id
    );

    // short labels are used for
    let mut short_labels = vec![
        ("id", illust.id.to_string()),
        ("user_id", illust.user.id.to_string()),
    ];

    let uptime = Utc::now().timestamp() - illust.create_date.timestamp();

    gauge!("pixiv_illust_views", &short_labels).set(illust.total_view as f64);
    gauge!("pixiv_illust_bookmarks", &short_labels).set(illust.total_bookmarks as f64);
    gauge!("pixiv_illust_comments", &short_labels).set(illust.total_comments.unwrap_or(0) as f64);
    gauge!("pixiv_illust_uptime", &short_labels).set(uptime as f64);
    gauge!("pixiv_illust_page_count", &short_labels).set(illust.page_count as f64);

    for tag in &illust.tags {
        short_labels.push(("tag", tag.name.clone()));
        gauge!("pixiv_illust_tag", &short_labels).set(1.0);
        short_labels.pop();
    }

    let long_labels = vec![
        ("id", illust.id.to_string()),
        ("user_id", illust.user.id.to_string()),
        ("user_name", illust.user.name.to_string()),
        ("type", illust.type_.to_string()),
        ("title", illust.title.to_string()),
        ("caption", illust.caption.to_string()),
        ("create_date", illust.create_date.to_string()),
        ("ai_type", illust.illust_ai_type.to_string()),
        ("visible", illust.visible.to_string()),
        ("is_muted", illust.is_muted.to_string()),
    ];

    gauge!("pixiv_illust_info", &long_labels).set(1.0);
}
