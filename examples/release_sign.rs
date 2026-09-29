//! Signing Cairn's release downloads, so Cairn can check that an update
//! really comes from its maintainers before installing it.
//!
//! ```text
//! cargo run --example release_sign -- keygen <folder>
//!     Make a new key pair: <folder>/cairn-release.key (secret: keep it safe,
//!     and store it as the MINISIGN_SECRET_KEY secret on GitHub) and
//!     <folder>/cairn-release.pub. Prints the public key for src/update.rs.
//!
//! cargo run --example release_sign -- sign <file>...
//!     Write <file>.minisig for each file, using the secret key in the
//!     MINISIGN_SECRET_KEY environment variable. The signed trusted comment
//!     is "file:<file name>", which Cairn checks.
//! ```
//!
//! This is an example (not part of cairn.exe) because only the release
//! workflow and the maintainer need it.

use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use minisign::{KeyPair, SecretKeyBox};

fn keygen(dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    let secret_path = dir.join("cairn-release.key");
    if secret_path.exists() {
        return Err(format!(
            "{} already exists. It was not replaced.",
            secret_path.display()
        )
        .into());
    }
    let pair = KeyPair::generate_unencrypted_keypair()?;
    let secret = pair.sk.to_box(Some("Cairn release signing key (secret)"))?;
    fs::write(&secret_path, secret.into_string())?;
    fs::write(
        dir.join("cairn-release.pub"),
        pair.pk.to_box()?.into_string(),
    )?;
    println!("Secret key: {}", secret_path.display());
    println!("Public key: {}", pair.pk.to_base64());
    Ok(())
}

fn sign(files: &[String]) -> Result<(), Box<dyn Error>> {
    let text = std::env::var("MINISIGN_SECRET_KEY")
        .map_err(|_| "Set MINISIGN_SECRET_KEY to the contents of cairn-release.key.")?;
    let secret = SecretKeyBox::from_string(text.trim())?.into_unencrypted_secret_key()?;
    for file in files {
        let name = Path::new(file)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Unusable file name.")?;
        let signature = minisign::sign(
            None,
            &secret,
            File::open(file)?,
            Some(&format!("file:{name}")),
            Some("signature from the Cairn release key"),
        )?;
        fs::write(format!("{file}.minisig"), signature.into_string())?;
        println!("Signed {name}");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") if args.len() == 2 => keygen(&PathBuf::from(&args[1])),
        Some("sign") if args.len() > 1 => sign(&args[1..]),
        _ => Err("usage: release_sign keygen <folder> | release_sign sign <file>...".into()),
    }
}
