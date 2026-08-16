//! Assets carried inside the executable, for an engine that reads them off a disk.
//!
//! A simulation built here is meant to be one file. You hand somebody `moebius3.exe` and it runs:
//! no directory to keep beside it, nothing to unzip, and no path on your machine baked into it.
//!
//! Fulcrum on its own does not do that. `AssetServer::new(root)` takes a directory, and games are
//! written as `AssetServer::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"))`, which bakes an
//! absolute path into the binary at build time. That is the right answer while the source tree is
//! there and the wrong one everywhere else: move the repository, build in a worktree, or give the
//! executable to somebody, and it points at a directory that is not on their disk.
//!
//! So the assets come along inside the binary, and this crate is the seam that makes them
//! readable. It sits between the simulations and the engine and does not patch it: everything
//! here is built on `AssetServer::new` and `AssetServer::mount`, both of which the engine already
//! offers.
//!
//! # What happens when
//!
//! [`assets!`] decides between two cases, and the decision is which directory is really there:
//!
//! - **The source tree is present**, which is every `cargo run` and every `cargo test`. Its
//!   `assets` directory is mounted and nothing else is done. This is what the engine would have
//!   done unaided, so hot reload works exactly as it always has and nothing is written anywhere.
//! - **It is not**, which is what a downloaded executable finds. The compiled-in copy is written
//!   out to a directory under the system temp once, and that is mounted instead.
//!
//! Either way, an `assets` directory sitting next to the running executable is mounted on top, so
//! anything you drop there wins. That is the seam for handing somebody a different sprite without
//! rebuilding.
//!
//! # Why it is written out rather than read from memory
//!
//! Every asset read in the engine ends at `Vfs::read`, which calls `std::fs::read`, and listing
//! ends at `std::fs::read_dir`. There is no trait behind either and no mount that is not a
//! directory, so bytes held in the binary cannot be handed to a loader as they are. The choice is
//! to change the engine or to give it a directory, and the engine is upstream's.
//!
//! The cost is one write of a few megabytes, once per version of a given simulation, on a machine
//! that has no copy of the source. It never happens while you are working. The thing it buys is
//! that the executable is still one file: the unpacked copy is a cache that can be deleted at any
//! moment and will be written again, not something a user has to be given or told about.

use std::path::{Path, PathBuf};

use fulcrum_asset::AssetServer;

// Re-exported so a simulation does not have to depend on `include_dir` to use [`assets!`]. The
// crate has to be reachable by that name, because the code the macro generates says `include_dir`.
pub use ::include_dir;
pub use include_dir::Dir;

/// What a directory of assets beside the executable is called, and the name of its mount.
pub const BESIDE: &str = "assets";

/// The directory under the system temp that unpacked copies go in.
const CACHE: &str = "simulation-and-simulacra";

/// Written into an unpacked directory once every file is in it.
///
/// The unpacking is not atomic, so a run killed halfway through it leaves a directory with some of
/// the assets in it. Without this, the next run would find that directory, believe it, and fail to
/// read whatever had not been written yet. The marker goes in last, so a directory without one is
/// treated as absent and written again.
const DONE: &str = ".unpacked";

/// The `assets` directory next to the running executable, if there is one.
///
/// `None` when the directory is absent, and also when the platform will not say where the
/// executable is. Both mean the same thing here: there is nothing to mount.
pub fn beside_executable() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let beside = exe.parent()?.join(BESIDE);
    beside.is_dir().then_some(beside)
}

/// The asset server a simulation runs on. Written as [`assets!`] rather than called directly,
/// because the compiled-in copy has to be made in the simulation's own crate.
///
/// `source` is the assets directory in the source tree, as an absolute path fixed at build time.
/// `compiled` is the same directory as bytes in the binary. `name` tells one simulation's unpacked
/// copy from another's.
pub fn server(source: &str, compiled: &'static Dir<'static>, name: &str) -> AssetServer {
    let source = Path::new(source);
    let mut server = if source.is_dir() {
        AssetServer::new(source)
    } else {
        AssetServer::new(unpacked(compiled, name))
    };
    if let Some(beside) = beside_executable() {
        server.mount(BESIDE, beside);
    }
    server
}

/// Where the compiled-in copy has been written, writing it first if it is not there yet.
///
/// The directory is named for what is in it: two simulations do not collide, and neither do two
/// versions of one, so an executable that has been rebuilt never reads the previous build's
/// assets out of a stale cache.
fn unpacked(compiled: &'static Dir<'static>, name: &str) -> PathBuf {
    let stamp = fingerprint(compiled);
    let dir = std::env::temp_dir()
        .join(CACHE)
        .join(format!("{name}-{stamp:016x}"));
    if dir.join(DONE).is_file() {
        return dir;
    }
    // Whatever is there is a half-written copy from a run that died, since a whole one has the
    // marker. Take it away rather than writing over it: a file that was renamed between the two
    // builds would otherwise survive and shadow nothing.
    let _ = std::fs::remove_dir_all(&dir);
    write_out(compiled, &dir);
    let _ = std::fs::write(dir.join(DONE), format!("{stamp:016x}"));
    dir
}

/// Every file of a compiled-in directory, written under `root` at the path it was compiled with.
fn write_out(dir: &Dir<'_>, root: &Path) {
    for file in dir.files() {
        let out = root.join(file.path());
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(out, file.contents());
    }
    for inner in dir.dirs() {
        write_out(inner, root);
    }
}

/// A number that changes whenever any of the compiled-in assets does.
///
/// Over the contents and not only over the names and sizes, because the case this is guarding
/// against is a rebuilt executable finding the previous build's unpacked copy, and an edited
/// sprite is very often the same length as the one it replaced.
fn fingerprint(dir: &'static Dir<'static>) -> u64 {
    let mut hash = xxhash_rust::xxh3::Xxh3::new();
    fold(dir, &mut hash);
    hash.digest()
}

/// Walks in the order `include_dir` holds them, which is fixed at compile time, so the same
/// binary always reaches the same number.
fn fold(dir: &Dir<'_>, hash: &mut xxhash_rust::xxh3::Xxh3) {
    for file in dir.files() {
        hash.update(file.path().to_string_lossy().as_bytes());
        hash.update(file.contents());
    }
    for inner in dir.dirs() {
        fold(inner, hash);
    }
}

/// The asset server for this simulation, with its `assets` directory compiled into the binary.
///
/// Written once, in `main`:
///
/// ```ignore
/// use fulcrum::prelude::*;
/// use simulacra_assets::assets;
///
/// Fulcrum::with_config(config)
///     .insert_resource(assets!())
///     .with_plugin(DefaultPlugins)
///     .run();
/// ```
///
/// It is a macro rather than a function because the compiled-in copy has to be made in the
/// calling crate: `include_dir!` and `env!("CARGO_MANIFEST_DIR")` both answer for the crate being
/// compiled, and only the simulation knows where its own assets are.
#[macro_export]
macro_rules! assets {
    () => {{
        // In scope by that name because the code `include_dir!` generates says `include_dir`.
        use $crate::include_dir;
        static COMPILED: $crate::Dir<'static> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/assets");
        $crate::server(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets"),
            &COMPILED,
            env!("CARGO_PKG_NAME"),
        )
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use include_dir::include_dir;

    // This crate's own `tests/tree`, compiled in, which is what the simulations do with `assets`.
    static TREE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/tests/tree");

    #[test]
    fn every_file_comes_out_where_it_went_in() {
        let root = std::env::temp_dir().join("simulacra-assets-write-out");
        let _ = std::fs::remove_dir_all(&root);
        write_out(&TREE, &root);

        assert_eq!(std::fs::read(root.join("top.txt")).unwrap(), b"top\n");
        assert_eq!(
            std::fs::read(root.join("sprites/hero.txt")).unwrap(),
            b"hero\n",
            "a file in a subdirectory did not survive the walk"
        );
    }

    #[test]
    fn the_second_run_does_not_write_again() {
        let dir = unpacked(&TREE, "twice");
        assert!(dir.join(DONE).is_file(), "the marker was never written");
        let written = std::fs::metadata(dir.join("top.txt"))
            .unwrap()
            .modified()
            .unwrap();

        let again = unpacked(&TREE, "twice");
        assert_eq!(dir, again, "the same assets landed in two directories");
        assert_eq!(
            written,
            std::fs::metadata(again.join("top.txt"))
                .unwrap()
                .modified()
                .unwrap(),
            "the copy was written a second time"
        );
    }

    #[test]
    fn a_half_written_copy_is_not_believed() {
        // What a run killed during the unpacking leaves behind: a directory with some of the
        // files in it and no marker. The next run has to write it again rather than read it.
        let dir = unpacked(&TREE, "partial");
        std::fs::remove_file(dir.join(DONE)).unwrap();
        std::fs::remove_file(dir.join("sprites/hero.txt")).unwrap();

        let again = unpacked(&TREE, "partial");
        assert_eq!(dir, again);
        assert_eq!(
            std::fs::read(again.join("sprites/hero.txt")).unwrap(),
            b"hero\n",
            "the missing file was never put back"
        );
    }

    #[test]
    fn the_name_follows_the_contents() {
        // The guard against a rebuilt executable reading the previous build's assets. Two
        // different trees cannot land in one directory, whatever they are called.
        static OTHER: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src");
        assert_ne!(
            fingerprint(&TREE),
            fingerprint(&OTHER),
            "two different sets of assets have the same fingerprint"
        );
    }

    #[test]
    fn the_source_tree_wins_while_you_are_working() {
        // Under `cargo test` the source tree is right there, so nothing is unpacked and the
        // engine is left doing what it would have done unaided.
        let source = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/tree");
        let server = server(source, &TREE, "source-tree");
        assert_eq!(
            server.read_bytes("top.txt").unwrap(),
            b"top\n",
            "the assets did not come from the source tree"
        );
        assert!(
            server.roots().iter().any(|root| root == Path::new(source)),
            "the source tree was not the mount"
        );
    }

    #[test]
    fn the_engine_reads_the_compiled_copy_when_there_is_no_source_tree() {
        // The whole point, and the case no other test covers: a downloaded executable, on a
        // machine with none of the build directories on it. The source path it was built with
        // does not exist, so the assets have to come out of the binary, and they have to come out
        // through the engine's own reader rather than through anything in this crate.
        let server = server("C:/no/such/directory/assets", &TREE, "distributed");
        assert_eq!(
            server.read_bytes("top.txt").unwrap(),
            b"top\n",
            "the engine could not read an asset out of the compiled-in copy"
        );
        assert_eq!(
            server.read_bytes("sprites/hero.txt").unwrap(),
            b"hero\n",
            "a nested asset did not survive"
        );
        // And `list` too, which is a different call into the engine: `read` walks files and this
        // walks directories. The data-driven pieces discover their content with it.
        assert_eq!(server.list("sprites", "txt"), vec!["sprites/hero.txt"]);
    }
}
