//! Native macOS Dock menu. Extend the existing delegate without replacing Tauri's
//! delegate, which also handles Finder imports and application lifecycle events.

use std::{cell::OnceCell, ptr};

use objc2::{
    ffi::class_addMethod,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Imp, ProtocolObject, Sel},
    sel, MainThreadOnly,
};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSMenu, NSMenuItem};
use objc2_foundation::{ns_string, MainThreadMarker, NSString};

use crate::app_navigation::AppView;

const ITEMS: [(AppView, &str); 3] = [
    (AppView::Convert, "Convert"),
    (AppView::Presets, "Presets"),
    (AppView::WatchedFolders, "Watched Folders"),
];

struct DockMenuState {
    menu: Retained<NSMenu>,
    app: tauri::AppHandle,
    // NSMenuItem targets are not retained by AppKit.
    _delegate: Retained<ProtocolObject<dyn NSApplicationDelegate>>,
}

thread_local! {
    static DOCK_MENU: OnceCell<DockMenuState> = const { OnceCell::new() };
}

pub(crate) fn install(app: &tauri::AppHandle) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("Dock menu must be installed on the main thread.")?;
    let native_app = NSApplication::sharedApplication(mtm);
    let delegate = native_app
        .delegate()
        .ok_or("The macOS application delegate is unavailable.")?;
    let delegate_object: &AnyObject = (*delegate).as_ref();
    let class = delegate_object.class();
    let menu_selector = sel!(applicationDockMenu:);
    let action_selector = sel!(bulkPixelNavigate:);

    if class.instance_method(menu_selector).is_some()
        || class.instance_method(action_selector).is_some()
    {
        return Err("The macOS application delegate already handles the Dock menu.".into());
    }

    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    for (index, (_, label)) in ITEMS.iter().enumerate() {
        // SAFETY: The selector accepts one NSMenuItem and returns void. Its target
        // is the existing delegate, retained in DOCK_MENU for the process lifetime.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(label),
                Some(action_selector),
                ns_string!(""),
            )
        };
        item.setTag(index as isize);
        unsafe { item.setTarget(Some(delegate_object)) };
        menu.addItem(&item);
    }

    DOCK_MENU.with(|state| {
        state
            .set(DockMenuState {
                menu,
                app: app.clone(),
                _delegate: delegate.clone(),
            })
            .map_err(|_| "The Dock menu is already installed.")
    })?;

    // SAFETY: Only optional methods absent from the existing delegate are added.
    // The function signatures and Objective-C type encodings match exactly:
    // object-returning applicationDockMenu:(object), void bulkPixelNavigate:(object).
    // AppKit invokes both on the main thread; all objects stay in thread-local storage.
    unsafe {
        let menu_imp = std::mem::transmute::<
            extern "C-unwind" fn(&AnyObject, Sel, &NSApplication) -> *mut NSMenu,
            Imp,
        >(application_dock_menu);
        let action_imp = std::mem::transmute::<
            extern "C-unwind" fn(&AnyObject, Sel, &NSMenuItem),
            Imp,
        >(navigate);
        let class = class as *const AnyClass as *mut AnyClass;
        if !class_addMethod(class, menu_selector, menu_imp, c"@@:@".as_ptr()).as_bool()
            || !class_addMethod(class, action_selector, action_imp, c"v@:@".as_ptr()).as_bool()
        {
            return Err("Unable to register the native Dock menu handlers.".into());
        }
    }
    // Refresh AppKit's optional-delegate-method cache with the same delegate.
    native_app.setDelegate(Some(&delegate));
    Ok(())
}

extern "C-unwind" fn application_dock_menu(
    _delegate: &AnyObject,
    _selector: Sel,
    _app: &NSApplication,
) -> *mut NSMenu {
    DOCK_MENU.with(|state| {
        state.get().map_or(ptr::null_mut(), |state| {
            Retained::as_ptr(&state.menu).cast_mut()
        })
    })
}

extern "C-unwind" fn navigate(_delegate: &AnyObject, _selector: Sel, item: &NSMenuItem) {
    let Some((view, _)) = usize::try_from(item.tag())
        .ok()
        .and_then(|index| ITEMS.get(index))
    else {
        return;
    };
    DOCK_MENU.with(|state| {
        if let Some(state) = state.get() {
            crate::navigate_to_view(&state.app, *view);
        }
    });
}
