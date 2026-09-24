fn main() {
  // Ensure frontend asset changes trigger a rebuild in release.
  // These directives tell Cargo to re-run this build script (and thus
  // re-embed all dist/ assets) whenever any of these files change.
  println!("cargo:rerun-if-changed=../dist");
  println!("cargo:rerun-if-changed=../dist/index.html");
  println!("cargo:rerun-if-changed=../dist/settings.html");
  println!("cargo:rerun-if-changed=../dist/index.js");
  tauri_build::build();
}
