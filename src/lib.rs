//! The `Crawly` web crawler efficiently fetches and stores content from web pages.
//! It respects `robots.txt` guidelines and handles rate limits.

use anyhow::Result;
use futures::future::join_all;
use indexmap::IndexMap;
use reqwest::{Client, Url};
use robotstxt::DefaultMatcher;
use scraper::{Html, Selector};
use std::collections::HashSet;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{sleep, Duration};

const USER_AGENT: &str = "CrawlyRustCrawler";

// Default configuration constants.
const MAX_DEPTH: usize = 5;
const MAX_PAGES: usize = 15;
const MAX_CONCURRENT_REQUESTS: usize = 1_000;
const RATE_LIMIT_WAIT_SECONDS: u64 = 1;

/// Cache structure to store information about a domain's `robots.txt`.
struct RobotsCache {
    content: String,
    crawl_delay: Option<u64>, // Delay specified by the `robots.txt`.
}

/// Configuration parameters for the `Crawler`.
/// Defines bounds and behaviors for the crawling process.
struct CrawlerConfig {
    max_depth: usize,
    max_pages: usize,
    max_concurrent_requests: usize,
    rate_limit_wait_seconds: u64,
}

impl Default for CrawlerConfig {
    /// Default configuration for the crawler.
    fn default() -> Self {
        Self {
            max_depth: MAX_DEPTH,
            max_pages: MAX_PAGES,
            max_concurrent_requests: MAX_CONCURRENT_REQUESTS,
            rate_limit_wait_seconds: RATE_LIMIT_WAIT_SECONDS,
        }
    }
}

/// Builder pattern for `Crawler`. Allows for customizable configurations.
pub struct CrawlerBuilder {
    config: CrawlerConfig,
}

impl Default for CrawlerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CrawlerBuilder {
    /// Initializes a new builder with default configuration.
    pub fn new() -> Self {
        CrawlerBuilder {
            config: CrawlerConfig::default(),
        }
    }

    /// Set a specific maximum depth for the crawl.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.config.max_depth = depth;
        self
    }

    /// Set a specific maximum number of pages to fetch.
    pub fn with_max_pages(mut self, pages: usize) -> Self {
        self.config.max_pages = pages;
        self
    }

    /// Set a limit for concurrent requests.
    pub fn with_max_concurrent_requests(mut self, requests: usize) -> Self {
        self.config.max_concurrent_requests = requests;
        self
    }

    /// Define a rate limit delay in seconds.
    pub fn with_rate_limit_wait_seconds(mut self, seconds: u64) -> Self {
        self.config.rate_limit_wait_seconds = seconds;
        self
    }

    /// Consumes the builder and returns a configured `Crawler` instance.
    pub fn build(self) -> Result<Crawler> {
        Crawler::from_config(self.config)
    }
}

/// Main structure for the `Crawler` containing necessary utilities and caches.
pub struct Crawler {
    config: CrawlerConfig, // Configuration parameters.
    client: Client,        // HTTP client to make web requests.
    robots_cache: RwLock<IndexMap<String, RobotsCache>>, // Cache for `robots.txt` per domain.
}

impl Crawler {
    /// Initializes the crawler with a given configuration.
    fn from_config(config: CrawlerConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: Client::builder().user_agent(USER_AGENT).build()?,
            robots_cache: RwLock::new(IndexMap::new()),
        })
    }

    /// Initializes a new `Crawler` instance with the default configuration.
    pub fn new() -> Result<Self> {
        Self::from_config(CrawlerConfig::default())
    }

    /// Asynchronously crawls a URL. Honors `robots.txt`, maintains state about visited URLs,
    /// and manages rate limits and concurrency.
    #[async_recursion::async_recursion]
    async fn crawl(
        &self,
        semaphore: &Semaphore, // Rate limiting and concurrency management.
        url: Url,
        depth: usize,                            // Current depth of the crawl.
        visited: &RwLock<HashSet<Url>>,          // Set of visited URLs to avoid redundancy.
        content: &RwLock<IndexMap<Url, String>>, // Collected content per URL.
    ) -> Result<()> {
        // Recursion base cases.
        if depth > self.config.max_depth
            || visited.read().await.len() > self.config.max_pages
            || visited.read().await.contains(&url)
        {
            return Ok(());
        }

        let permit = semaphore.acquire().await;

        // Fetch and handle `robots.txt` for the domain.
        let robots_url = format!(
            "{}://{}/robots.txt",
            url.scheme(),
            url.host().ok_or(anyhow::anyhow!("Host not found."))?
        );
        let domain = url.domain().unwrap_or_default().to_string();

        let mut robots_cache = self.robots_cache.write().await;

        // Get cached robots info or fetch if not cached.
        let (robots_content, delay_seconds) = if let Some(info) = robots_cache.get(&domain) {
            (
                info.content.clone(),
                info.crawl_delay.unwrap_or(RATE_LIMIT_WAIT_SECONDS),
            )
        } else {
            let robots_content = self.client.get(&robots_url).send().await?.text().await?;

            let delay_seconds = robots_content
                .lines()
                .filter_map(|line| {
                    if line.contains("Crawl-delay") {
                        line.split(':').last()?.trim().parse().ok()
                    } else {
                        None
                    }
                })
                .next()
                .unwrap_or(RATE_LIMIT_WAIT_SECONDS);

            robots_cache.insert(
                domain.clone(),
                RobotsCache {
                    content: robots_content.clone(),
                    crawl_delay: Some(delay_seconds),
                },
            );
            (robots_content, delay_seconds)
        };

        drop(robots_cache);

        // Respect the crawl delay specified by `robots.txt`.
        sleep(Duration::from_secs(delay_seconds)).await;

        // Check permission from `robots.txt` before proceeding.
        if !DefaultMatcher::default().one_agent_allowed_by_robots(
            &robots_content,
            USER_AGENT,
            url.as_str(),
        ) {
            return Ok(());
        }

        // Fetch the page content.
        let html = self.client.get(url.clone()).send().await?.text().await?;
        content.write().await.insert(url.clone(), html.clone());

        // Explicitly dropping the permit to free up concurrency slot.
        drop(permit);

        visited.write().await.insert(url.clone());

        // Continue crawling by processing extracted links recursively.
        let _ =
            join_all(Self::extract_links(&html)?.into_iter().filter_map(
                |link| match url.join(&link) {
                    Ok(url) => Some(self.crawl(semaphore, url, depth + 1, visited, content)),
                    Err(_) => None,
                },
            ))
            .await;

        Ok(())
    }

    /// Extracts hyperlinks from given HTML content.
    fn extract_links(content: &str) -> Result<Vec<String>> {
        let document = Html::parse_document(content);
        let selector = Selector::parse("a").map_err(|error| anyhow::anyhow!("{:?}", error))?;

        Ok(document
            .select(&selector)
            .filter_map(|element| element.value().attr("href").map(|href| href.to_string()))
            .collect())
    }

    /// Initiates the crawling process from a specified root URL.
    ///
    /// Returns a map of visited URLs and their corresponding HTML content.
    pub async fn start<S: AsRef<str>>(&self, url: S) -> Result<IndexMap<Url, String>> {
        let root_url = Url::parse(url.as_ref())?;

        let semaphore = Semaphore::new(self.config.max_concurrent_requests);
        let visited = RwLock::new(HashSet::new());
        let content = RwLock::new(IndexMap::new());

        self.crawl(&semaphore, root_url, 0, &visited, &content)
            .await?;

        Ok(content.into_inner())
    }
}
