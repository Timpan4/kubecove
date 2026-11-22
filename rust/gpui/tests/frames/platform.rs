//! Headless windows with production GPUI frame scheduling.
//! The SDK benchmark platform creates TestWindows but leaves App in Production
//! mode. Retain their public frame callbacks before Window takes ownership.

use gpui_kit::*;
use std::{
    cell::RefCell,
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
struct ScheduledWindow {
    handle: AnyWindowHandle,
    deliver: Rc<dyn Fn() -> bool>,
}

pub struct FramePlatform {
    inner: Rc<dyn Platform>,
    windows: RefCell<Vec<ScheduledWindow>>,
}

impl FramePlatform {
    pub fn new(gpu: bool) -> Rc<Self> {
        let renderer = gpu.then(|| {
            Box::new(gpui_kit::platform::current_headless_renderer)
                as Box<dyn Fn() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>>>
        });
        Rc::new(Self {
            inner: bench_platform(
                renderer,
                gpui_kit::platform::current_platform(true).text_system(),
            ),
            windows: RefCell::new(Vec::new()),
        })
    }

    pub fn callbacks(&self, open: &[AnyWindowHandle]) -> Vec<Rc<dyn Fn() -> bool>> {
        let mut windows = self.windows.borrow_mut();
        windows.retain(|window| {
            open.iter()
                .any(|handle| handle.window_id() == window.handle.window_id())
        });
        windows
            .iter()
            .map(|window| window.deliver.clone())
            .collect()
    }
}

impl Platform for FramePlatform {
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        let mut window = self.inner.open_window(handle, options)?;
        let scheduled = window.as_test().expect("SDK benchmark window").clone();
        self.windows.borrow_mut().push(ScheduledWindow {
            handle,
            deliver: Rc::new(move || scheduled.simulate_scheduled_frame()),
        });
        Ok(window)
    }

    fn platform_info(&self) -> PlatformInfo {
        self.inner.platform_info()
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.inner.background_executor()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.inner.foreground_executor()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.inner.text_system()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
    }

    fn quit(&self) {
        self.inner.quit()
    }

    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<OsString>) {
        self.inner.restart(binary_path, arguments)
    }

    fn activate(&self, ignoring_other_apps: bool) {
        self.inner.activate(ignoring_other_apps)
    }

    fn hide(&self) {
        self.inner.hide()
    }

    fn hide_other_apps(&self) {
        self.inner.hide_other_apps()
    }

    fn unhide_other_apps(&self) {
        self.inner.unhide_other_apps()
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.inner.displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.inner.primary_display()
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_displays_changed(callback)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.inner.active_window()
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.inner.window_appearance()
    }

    fn open_url(&self, url: &str) {
        self.inner.open_url(url)
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.inner.on_open_urls(callback)
    }

    fn register_url_scheme(&self, url: &str) -> Task<Result<()>> {
        self.inner.register_url_scheme(url)
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> futures::channel::oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        self.inner.prompt_for_paths(options)
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> futures::channel::oneshot::Receiver<Result<Option<PathBuf>>> {
        self.inner.prompt_for_new_path(directory, suggested_name)
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.inner.can_select_mixed_files_and_dirs()
    }

    fn reveal_path(&self, path: &Path) {
        self.inner.reveal_path(path)
    }

    fn open_with_system(&self, path: &Path) {
        self.inner.open_with_system(path)
    }

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.inner.on_quit(callback)
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_reopen(callback)
    }

    fn on_system_sleep(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_system_sleep(callback)
    }

    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_system_wake(callback)
    }

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        self.inner.set_menus(menus, keymap)
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        self.inner.set_dock_menu(menu, keymap)
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.inner.on_app_menu_action(callback)
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_will_open_app_menu(callback)
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.inner.on_validate_app_menu_command(callback)
    }

    fn thermal_state(&self) -> ThermalState {
        self.inner.thermal_state()
    }

    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_thermal_state_change(callback)
    }

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        self.inner.prevent_idle_sleep(reason)
    }

    fn app_path(&self) -> Result<PathBuf> {
        self.inner.app_path()
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.inner.path_for_auxiliary_executable(name)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        self.inner.set_cursor_style(style)
    }

    fn hide_cursor_until_mouse_moves(&self) {
        self.inner.hide_cursor_until_mouse_moves()
    }

    fn is_cursor_visible(&self) -> bool {
        self.inner.is_cursor_visible()
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        self.inner.should_auto_hide_scrollbars()
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.inner.read_from_clipboard()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.inner.write_to_clipboard(item)
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.inner.read_from_primary()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        self.inner.write_to_primary(item)
    }

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.inner.read_from_find_pasteboard()
    }

    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.inner.write_to_find_pasteboard(item)
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        self.inner.write_credentials(url, username, password)
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        self.inner.read_credentials(url)
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        self.inner.delete_credentials(url)
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.inner.keyboard_layout()
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.inner.keyboard_mapper()
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_keyboard_layout_change(callback)
    }
}
