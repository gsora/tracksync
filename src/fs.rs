use async_std::channel::{Receiver, Sender};

/// Traverses the file system from the given path.
pub async fn traverse(path: &str) -> Receiver<Result<String, std::io::Error>> {
    let (tx, rx): (
        Sender<Result<String, std::io::Error>>,
        Receiver<Result<String, std::io::Error>>,
    ) = async_std::channel::unbounded();

    let path = path.to_owned();

    async_std::task::spawn(async move { traverse_inner(path, tx).await });

    rx
}

async fn traverse_inner(path: String, tx: Sender<Result<String, std::io::Error>>) {
    for maybe_path in walkdir::WalkDir::new(path) {
        let path = match maybe_path {
            Ok(p) => p,
            Err(e) => {
                tx.send(Err(e.into())).await.unwrap();
                tx.close();
                return;
            }
        };

        let meta = match path.metadata() {
            Ok(m) => m,
            Err(e) => {
                tx.send(Err(e.into())).await.unwrap();
                tx.close();
                return;
            }
        };

        let path_str = path.path().to_str().unwrap().to_string();

        match meta.is_dir() {
            true => {}
            false => {
                if is_music(&path_str) {
                    tx.send(Ok(path_str)).await.unwrap();
                }
            }
        }
    }

    tx.close();
}

/// Returns true if name has one of the supported music file extension.
fn is_music(name: &String) -> bool {
    let formats = ["flac", "mp3", "ogg", "mp4", "m4a"];

    formats
        .into_iter()
        .filter_map(|format| name.ends_with(&format!(".{format}")).then_some(true))
        .collect::<Vec<bool>>()
        .into_iter()
        .find(|x| *x == true)
        .is_some()
}
