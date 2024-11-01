use crate::db;
use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::collections::{hash_map, hash_set};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Directory in which tunesdirector will store its local database.
    #[arg(short, long, default_value_t = db::default_database_dir().to_str().unwrap().to_owned())]
    pub database_path: String,
}

pub async fn run(args: Args) -> Result<()> {
    // Open a database
    let db = db::Instance::new(&args.database_path, false)
        .await
        .with_context(|| "Cannot open local database instance")?;

    let albums = db.albums().await.with_context(|| "Cannot fetch albums")?;

    // (Artist, album keywords)
    let keywords: Vec<(String, Vec<String>)> = albums
        .into_iter()
        .map(|a| {
            let album = split_after_parenthesis(a.title.clone());

            (
                clean(a.artist.clone()),
                album
                    .split_whitespace()
                    .into_iter()
                    .filter_map(|word| {
                        if word.len() > 3 {
                            return Some(clean(word.to_owned()));
                        }

                        return None;
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let mut dedup: hash_set::HashSet<(String, String)> = hash_set::HashSet::new();

    for keyword in keywords {
        if keyword.1.len() == 0 {
            continue;
        }

        log::debug!("looking for: {:?}", keyword.1);
        // Album name : (track id, format)
        let albums = db
            .fuzzy_find_album(&keyword.1)
            .await
            .with_context(|| "Could not fuzzy find album")?
            .into_iter()
            .map(|e| {
                (
                    split_after_parenthesis(e.1),
                    (split_after_parenthesis(e.2), e.0),
                )
            })
            .collect::<hash_map::HashMap<_, _>>();

        log::debug!("found {} entries", albums.len());
        if albums.is_empty() {
            log::debug!("did not find any album for fuzzy query");
            continue;
        }

        if albums.len() <= 1 {
            log::debug!("found just one album for query, likely no dupe");
            continue;
        }

        let album_names: Vec<String> = albums.keys().into_iter().cloned().collect();

        for (album_name, metadata) in &albums {
            let mut options = album_names.clone();
            if let Some(pos) = options.iter().position(|x| **x == *album_name) {
                options.remove(pos);
            }

            let (format, _) = metadata;

            match similar_string::find_best_similarity(album_name.clone(), &options) {
                Some(res) => {
                    let (dupe_name, score) = res;
                    let an_trim = album_name.trim().to_string();

                    match dedup.get(&(an_trim.clone(), dupe_name.clone())) {
                        Some(_) => continue,
                        None => {
                            dedup.insert((an_trim.clone(), dupe_name.clone()));
                        }
                    };

                    if res.1 < 0.6 {
                        continue;
                    }

                    let dupe_meta = albums.get(&dupe_name).unwrap();

                    let dupe_path = db
                        .tracks_by_id(vec![dupe_meta.1.clone()])
                        .await
                        .with_context(|| "Cannot fetch dupe track")?
                        .first()
                        .unwrap()
                        .file_path
                        .clone();

                    // parse dupe path and get the directory containing it
                    let dupe_path = std::path::Path::new(&dupe_path).parent().unwrap();
                    let dupe_path = dupe_path.to_str().unwrap();

                    println!(
                        "Maybe duplicate:\n\t\"{}\": \"{}\" (confidence: {:.1}%) \n\tat path {}, format {}",
                        album_name.trim(),
                        dupe_name.trim(),
                        score * (100 as f64),
                        dupe_path,
                        format,
                    );
                }
                None => {}
            }
        }
    }

    let std_duplicates = db
        .duplicate_albums()
        .await
        .with_context(|| "Cannot fetch duplicate albums")?;

    for sd in std_duplicates {
        let (album, amt) = sd;
        let paths = db
            .album_paths(&album.title, &album.artist)
            .await
            .with_context(|| "Cannot fetch duplicate album")?;

        println!(r#"Found "{}" in {} formats:"#, album.title, amt);

        for ele in paths {
            let (path, ext) = ele;
            println!("\t {}: {}", path, ext);
        }
    }

    Ok(())
}

fn clean(s: String) -> String {
    s.replace("(", " ")
        .replace(")", " ")
        .replace(":", " ")
        .replace(r#"'"#, r#" "#)
        .replace(".", " ")
        .replace("<", " ")
        .replace(">", " ")
        .replace(",", " ")
        .replace("-", " ")
        .replace("[", " ")
        .replace("]", " ")
        .replace("?", " ")
        .replace("/", " ")
        .replace("!", " ")
}

fn split_after_parenthesis(s: String) -> String {
    let split: Vec<(usize, char)> = s.char_indices().filter(|e| e.1 == '(').collect();

    match split.len() {
        0 => s,
        _ => s.clone().split_at(split.first().unwrap().0).0.to_string(),
    }
}
