# 🕷️ crawly

A lightweight and efficient web crawler in Rust, optimized for concurrent scraping while respecting `robots.txt` rules.

[![Crates.io](https://img.shields.io/crates/v/crawly.svg)](https://crates.io/crates/crawly)
![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)
![Version](https://img.shields.io/badge/version-0.1.0-green)
[![Repository](https://img.shields.io/badge/github-repository-orange)](https://github.com/aichat-bot/crawly)
[![Homepage](https://img.shields.io/badge/homepage-aichat-brightgreen)](https://ai-chat.it)

## 🚀 Features

- **Concurrent crawling**: Takes advantage of concurrency for efficient scraping across multiple cores;
- **Respects `robots.txt`**: Automatically fetches and adheres to website scraping guidelines;
- **DFS algorithm**: Uses a depth-first search algorithm to crawl web links;
- **Customizable with Builder Pattern**: Tailor the depth of crawling, rate limits, and other parameters effortlessly;
- **Cloudflare's detection**: If the destination URL is hosted with Cloudflare and a mitigation is found, the URL will
  be skipped;
- **Built with Rust**: Guarantees memory safety and top-notch speed.

## 📦 Installation

Add `crawly` to your `Cargo.toml`:

```toml
[dependencies]
crawly = "^0.1"
```

## 🛠️ Usage

A simple usage example:

```rust
use anyhow::Result;
use crawly::Crawler;

#[tokio::main]
async fn main() -> Result<()> {
    let crawler = Crawler::new()?;
    let results = crawler.crawl_url("https://example.com").await?;

    for (url, content) in &results {
        println!("URL: {}\nContent: {}", url, content);
    }

    Ok(())
}
```

### Using the Builder

For more refined control over the crawler's behavior, the CrawlerBuilder comes in handy:

```rust
use anyhow::Result;
use crawly::CrawlerBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    let crawler = CrawlerBuilder::new()
        .with_max_depth(10)
        .with_max_pages(100)
        .with_max_concurrent_requests(50)
        .with_rate_limit_wait_seconds(2)
        .with_robots(true)
        .build()?;

    let results = crawler.start("https://www.example.com").await?;

    for (url, content) in &results {
        println!("URL: {}\nContent: {}", url, content);
    }

    Ok(())
}
```

## 🛡️ Cloudflare

This crate will detect Cloudflare hosted sites and if the header `cf-mitigated` is found, the URL will be skipped
without
throwing any error.

## 📜 Tracing

Every function is instrumented, also this crate will emit some DEBUG messages for better comprehending the crawling
flow.

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!

Feel free to check [issues page](https://github.com/aichat-bot/crawly/issues). You can also take a look at
the [contributing guide](CONTRIBUTING.md).

## 📝 License

This project is [MIT](LICENSE) licensed.

## 💌 Contact

- Author: Dario Cancelliere
- Email: dario@ai-chat.it
- Company Website: [https://ai-chat.it](https://ai-chat.it)
