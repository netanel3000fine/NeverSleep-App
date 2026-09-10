fn main() {
  // Ensure frontend asset changes trigger a rebuild in release.
  println!("cargo:rerun-if-changed=../dist");
  println!("cargo:rerun-if-changed=../dist/settings.html");
  tauri_build::build();
}
