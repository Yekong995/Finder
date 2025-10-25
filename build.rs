fn main() {
    compile_slint();
}

#[cfg(feature = "ui")]
fn compile_slint() {
    slint_build::compile("ui/main.slint").unwrap();
}

// 防止未启用 feature 时编译报错
#[cfg(not(feature = "ui"))]
fn compile_slint() {
    return;
}
