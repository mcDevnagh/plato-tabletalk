mod args;
mod plato;
mod settings;

use std::{
    fmt::Display,
    fs::{self, File},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};
use args::Args;
use chrono::{Datelike, Local, Month, NaiveDate, Utc};
use plato::notify;
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde_json::json;
use settings::Settings;

fn error<T: Display>(err: T) {
    plato::notify(&err.to_string());
    eprintln!("tabletalk: {err}");
}

fn run() -> Result<()> {
    let args = Args::new()?;
    let settings = Settings::load()?;
    if !args.online {
        if !args.wifi {
            plato::notify("Please enable WiFi to update TABLETALK");
        } else {
            plato::notify("Waiting for the network to come up");
        }
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
    }

    if !args.save_path.exists() {
        fs::create_dir(&args.save_path)?;
    }

    let client = Client::builder()
        .user_agent(format!("plato-tabletalk/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let now = Utc::now();
    let year_regex = Regex::new(r"\d{4}")?;
    let month_regex = Regex::new(r"[A-Za-z]+")?;
    let uuid_regex = Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")?;
    let res = client.get(settings.url).send()?;
    let html = res.text()?;
    let doc = Html::parse_document(&html);
    let selector = Selector::parse("a[href$=epub]").unwrap();
    let selection = doc.select(&selector);
    let mut any_error = false;
    let sigterm = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&sigterm))?;
    for (i, select) in selection.into_iter().enumerate() {
        if i >= settings.limit || sigterm.load(Ordering::Relaxed) {
            break;
        }

        if let Some(href) = select.attr("href") {
            let mut year = None;
            for cap in year_regex.captures_iter(href) {
                year = match cap.get(0).unwrap().as_str().parse::<i32>() {
                    Ok(year) if year > 2000 && year <= now.year() => Some(year),
                    _ => continue,
                };
                break;
            }
            let mut month = None;
            for cap in month_regex.captures_iter(href) {
                month = match cap.get(0).unwrap().as_str().parse::<Month>() {
                    Ok(month) => Some(month),
                    _ => continue,
                };
                break;
            }

            let (title, filename) = if let (Some(year), Some(month)) = (year, month) {
                let date = NaiveDate::from_ymd_opt(year, month.number_from_month(), 1)
                    .ok_or_else(|| anyhow!("{:#?} {year} cannot be represented", month))?;
                (
                    date.format("TABLETALK - %B %Y").to_string(),
                    date.format("tabletalk-%Y-%m.epub").to_string(),
                )
            } else {
                if year.is_none() {
                    any_error = true;
                    error(anyhow!("Could not parse year from {href}"))
                }
                if month.is_none() {
                    any_error = true;
                    error(anyhow!("Could not parse month from {href}"))
                }

                continue;
            };

            let filepath = args.save_path.join(&filename);
            if filepath.exists() {
                continue;
            }

            let mut file = match File::create(&filepath) {
                Ok(file) => file,
                Err(err) => {
                    any_error = true;
                    error(err);
                    continue;
                }
            };

            let response = client
                .get(href)
                .send()
                .and_then(|mut response| response.copy_to(&mut file));

            if let Err(err) = response {
                any_error = true;
                error(err);
                fs::remove_file(filepath).ok();
                continue;
            }

            let path = match filepath.strip_prefix(&args.library_path) {
                Ok(path) => path,
                Err(err) => {
                    any_error = true;
                    error(err);
                    continue;
                }
            };
            let event = json!({
                "type": "addDocument",
                "info": {
                    "title": &title,
                    "author": "Ligonier Ministries",
                    "year": year,
                    "publisher": "Ligonier Ministries",
                    "identifier": uuid_regex
                        .captures(href)
                        .and_then(|c| c.get(0))
                        .map(|c| c.as_str())
                        .unwrap_or(&filename),
                    "added": Local::now().naive_local(),
                    "file": {
                        "path": path,
                        "kind": "epub",
                        "size": file.metadata().ok().map_or(0, |m| m.len()),
                    }
                },
            });
            println!("{event}");
            notify(&format!("Added {title}"));
        }
    }

    if any_error {
        notify("TABLETALK updated with errors");
    } else {
        notify("TABLETALK successfully updated");
    }
    Ok(())
}

fn main() {
    log_panics::init();
    if let Err(err) = run() {
        error(err);
    }
}
