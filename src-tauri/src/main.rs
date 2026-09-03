// 正式构建时隐藏 Windows 控制台窗口，请勿删除。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    muse_tune_lib::run();
}
