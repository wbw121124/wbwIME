fn main() {
    slint_build::compile(
        "ui/candidate_window.slint",
    )
    .expect("slint build script failed");
}
