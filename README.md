# pixiv-exporter

Pixiv illustration metrics exporter for Prometheus. Scrapes view counts, bookmarks, comments, and other metadata for configured users and works, and exposes them as Prometheus gauges.

## Installation

```bash
cargo install pixiv-exporter
```

## Usage

### Start the exporter server

Serve metrics with a config file (e.g. `config.json`):

```bash
pixiv-exporter serve config.json
```

Metrics are exposed on the configured bind address and port (default `127.0.0.1:6825`). Set the `PIXIV_REFRESH_TOKEN` environment variable (or configure it in the config file) for Pixiv API authentication.

### Config subcommands

- **Print JSON schema** for the config file:
  ```bash
  pixiv-exporter config schema
  ```

- **Print default config**:
  ```bash
  pixiv-exporter config default
  ```

- **Validate a config file**:
  ```bash
  pixiv-exporter config check config.json
  ```

## Configuration example

```json
{
  "target": {
    "users": [12345678],
    "works": [98765432]
  },
  "scrape": {
    "scrape_interval": { "interval": 1800, "variance": 0.2 },
    "independent_item_interval": { "interval": 1.5, "variance": 0.1 },
    "user_item_interval": { "interval": 0.1, "variance": 0.1 }
  },
  "server": {
    "bind": "127.0.0.1",
    "port": 6825
  },
  "refresh_token": { "env": "PIXIV_REFRESH_TOKEN" }
}
```

- `target.users` / `target.works`: user IDs and illustration IDs to scrape.
- `refresh_token`: Pixiv refresh token; use `{ "env": "VAR_NAME" }` to read from an environment variable.
- `server.bind` / `server.port`: address and port for the HTTP metrics server.
- `scrape.*`: intervals for scraping cycles and per-item delays (seconds; variance is optional).

Run `pixiv-exporter config schema` for the full JSON schema.

## License

MIT. See [LICENSE](./LICENSE) for details.
