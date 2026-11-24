fn main() {
    #[cfg(windows)]
    windows::build()
}

#[cfg(windows)]
mod windows {
    pub fn build() {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
