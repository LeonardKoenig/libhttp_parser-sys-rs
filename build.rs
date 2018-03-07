extern crate bindgen;

// Mostly based on
// https://github.com/alexcrichton/curl-rust/blob/master/curl-sys/build.rs

use std::env;
use std::io::{self, Read};
use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::process::Command;
use std::path::{Path, PathBuf};

macro_rules! t {
    ($e: expr) => {
        match $e {
            Ok(n) => n,
            Err(e) => panic!("\n{} failed with {}\n", stringify!($e), e),
        }
    };
}

fn main() {
    let src = env::current_dir().unwrap().join("src/http-parser");
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Create out-of-tree build
    let build = dst.join("build");
    t!(fs::create_dir_all(&build));
    cp_r(&src, &build);
    run(
        gnu_make().current_dir(&build).arg("package").arg("library"),
        "make",
    );
    let mut mak_contents = String::new();
    t!(t!(File::open(src.join("Makefile"))).read_to_string(&mut mak_contents));

    // parse so version
    let sover_lines: Vec<&str> = mak_contents
        .lines()
        .filter(|s| {
            s.contains("SOMAJOR = ") || s.contains("SOMINOR = ") || s.contains("SOREV   = ")
        })
        .collect();
    let somajor = sover_lines[0]
        // [ "SOxxx", "=", "Y" ]
        .split(" ")
        .collect::<Vec<&str>>()
        .last().unwrap()
            .parse::<u8>().unwrap();
    let sominor = sover_lines[1]
        // [ "SOxxx", "=", "Y" ]
        .split(" ")
        .collect::<Vec<&str>>()
        .last().unwrap()
            .parse::<u8>().unwrap();
    let sorev = sover_lines[2]
        // [ "SOxxx", "=", "Y" ]
        .split(" ")
        .collect::<Vec<&str>>()
        .last().unwrap()
            .parse::<u8>().unwrap();

    // package
    t!(fs::create_dir_all(dst.join("include")));
    t!(fs::create_dir_all(dst.join("lib")));
    t!(fs::copy(
        build.join("libhttp_parser.a"),
        dst.join("lib/libhttp_parser.a")
    ));
    let libname = format!("libhttp_parser.so.{}.{}.{}", somajor, sominor, sorev);
    t!(fs::copy(
        build.join(&libname),
        dst.join("lib").join(&libname)
    ));
    t!(symlink(&libname, dst.join("lib").join("libhttp_parser.so")));

    // output information
    println!("cargo:rustc-link-lib=http_parser");
    println!("cargo:rustc-link-lib=static=http_parser");
    println!("cargo:rustc-link-search={}/lib", dst.to_string_lossy());
    println!("cargo:root={}", dst.to_string_lossy());
    println!("cargo:include={}/include", dst.to_string_lossy());

    // generate bindings
    let bindings = bindgen::Builder::default()
        .header(src.join("http_parser.h").to_string_lossy())
        .blacklist_type("max_align_t")
        .generate()
        .expect("Unable to generate bindings");
    bindings
        .write_to_file(dst.join("bindings.rs"))
        .expect("Couldn't write bindings");
}

fn gnu_make() -> Command {
    let cmd = if cfg!(target_os = "freebsd") {
        "gmake"
    } else {
        "make"
    };
    let mut cmd = Command::new(cmd);
    // We're using the MSYS make which doesn't work with the mingw32-make-style
    // MAKEFLAGS, so remove that from the env if present.
    if cfg!(windows) {
        cmd.env_remove("MAKEFLAGS").env_remove("MFLAGS");
    }
    return cmd;
}

fn run(cmd: &mut Command, program: &str) {
    println!("running: {:?}", cmd);
    let status = match cmd.status() {
        Ok(status) => status,
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            fail(&format!(
                "failed to execute command: {}\nIs `{}` \
                 not installed?",
                e, program
            ));
        }
        Err(e) => fail(&format!("failed to execute command: {}", e)),
    };
    if !status.success() {
        fail(&format!(
            "command did not execute successfully, got: {}",
            status
        ));
    }
}

fn fail(s: &str) -> ! {
    println!("\n\n{}\n\n", s);
    std::process::exit(1);
}

fn cp_r(dir: &Path, dst: &Path) {
    for entry in t!(fs::read_dir(dir)) {
        let entry = t!(entry);
        let path = entry.path();
        let dst = dst.join(path.file_name().unwrap());
        if t!(fs::metadata(&path)).is_file() {
            t!(fs::copy(path, dst));
        } else {
            t!(fs::create_dir_all(&dst));
            cp_r(&path, &dst);
        }
    }
}
