#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Suppress console window on release builds on Windows

use std::path::{Path, PathBuf};
use eframe::egui;
use image::DynamicImage;

// Enum definitions
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridSize {
    Grid2x2,
    Grid3x3,
    Grid4x4,
}

impl GridSize {
    fn to_dims(self) -> (usize, usize) {
        match self {
            Self::Grid2x2 => (2, 2),
            Self::Grid3x3 => (3, 3),
            Self::Grid4x4 => (4, 4),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Grid2x2 => "2x2",
            Self::Grid3x3 => "3x3",
            Self::Grid4x4 => "4x4",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillMode {
    CropAndFill,
    Fit,
}

// Windows Printing Integration
#[cfg(target_os = "windows")]
mod win_print {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::*,
        Win32::Foundation::*,
        Win32::Graphics::Gdi::*,
        Win32::Graphics::Printing::*,
        Win32::Storage::Xps::*,
    };
    use image::DynamicImage;

    // RAII wrapper for Printer Handle to prevent resources leakage
    struct PrinterHandle(HANDLE);
    impl Drop for PrinterHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = ClosePrinter(self.0);
            }
        }
    }

    // RAII wrapper for Device Context (HDC)
    struct DcHandle(HDC);
    impl Drop for DcHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    pub fn get_default_printer() -> Option<String> {
        unsafe {
            let mut size: u32 = 0;
            let _ = GetDefaultPrinterW(PWSTR::null(), &mut size);
            if size == 0 {
                return None;
            }
            let mut buffer = vec![0u16; size as usize];
            if GetDefaultPrinterW(PWSTR(buffer.as_mut_ptr()), &mut size).as_bool() {
                let len = buffer.iter().position(|&x| x == 0).unwrap_or(buffer.len());
                Some(String::from_utf16_lossy(&buffer[..len]))
            } else {
                None
            }
        }
    }

    pub fn list_printers() -> Vec<String> {
        unsafe {
            let mut printers = Vec::new();
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let mut needed: u32 = 0;
            let mut returned: u32 = 0;
            
            let _ = EnumPrintersW(flags, None, 2, None, &mut needed, &mut returned);
            if needed == 0 {
                return printers;
            }
            
            let mut buffer = vec![0u8; needed as usize];
            if EnumPrintersW(
                flags,
                None,
                2,
                Some(&mut buffer),
                &mut needed,
                &mut returned,
            ).is_ok() {
                let info_slice = std::slice::from_raw_parts(
                    buffer.as_ptr() as *const PRINTER_INFO_2W,
                    returned as usize,
                );
                for info in info_slice {
                    if !info.pPrinterName.is_null() {
                        if let Ok(name) = info.pPrinterName.to_string() {
                            printers.push(name);
                        }
                    }
                }
            }
            
            if let Some(default) = get_default_printer() {
                if !printers.contains(&default) {
                    printers.insert(0, default);
                }
            }
            
            printers
        }
    }

    pub fn print_image(img: &DynamicImage, printer_name: &str, copies: usize, landscape: bool) -> std::result::Result<(), String> {
        unsafe {
            let printer_wide = to_wide(printer_name);
            let mut h_printer = HANDLE::default();
            
            OpenPrinterW(PCWSTR(printer_wide.as_ptr()), &mut h_printer, None)
                .map_err(|e| format!("Nelze otevřít tiskárnu: {}", e))?;
            
            // h_printer is now managed by RAII guard
            let _printer_guard = PrinterHandle(h_printer);
            
            let mut devmode: Option<*mut DEVMODEW> = None;
            let mut devmode_buffer = Vec::new();
            
            let needed = DocumentPropertiesW(
                HWND::default(),
                h_printer,
                PCWSTR(printer_wide.as_ptr()),
                None,
                None,
                0,
            );
            
            let mut copies_set_in_devmode = false;
            if needed > 0 {
                devmode_buffer.resize(needed as usize, 0u8);
                let res = DocumentPropertiesW(
                    HWND::default(),
                    h_printer,
                    PCWSTR(printer_wide.as_ptr()),
                    Some(devmode_buffer.as_mut_ptr() as *mut _),
                    None,
                    DM_OUT_BUFFER.0,
                );
                if res >= 0 {
                    let dm = devmode_buffer.as_mut_ptr() as *mut DEVMODEW;
                    (*dm).Anonymous1.Anonymous1.dmOrientation = if landscape { 2 } else { 1 };
                    (*dm).dmFields |= DM_ORIENTATION;
                    
                    (*dm).Anonymous1.Anonymous1.dmCopies = copies as i16;
                    (*dm).dmFields |= DM_COPIES;
                    
                    devmode = Some(dm);
                    copies_set_in_devmode = true;
                }
            }

            let devmode_const = devmode.map(|dm| dm as *const _);
            let hdc = CreateDCW(
                PCWSTR(to_wide("WINSPOOL").as_ptr()),
                PCWSTR(printer_wide.as_ptr()),
                None,
                devmode_const,
            );
            
            if hdc.is_invalid() {
                return Err("Nelze vytvořit grafický kontext tiskárny (DC).".to_string());
            }
            
            // hdc is now managed by RAII guard
            let _dc_guard = DcHandle(hdc);

            let doc_info = DOCINFOW {
                cbSize: std::mem::size_of::<DOCINFOW>() as i32,
                lpszDocName: PCWSTR(to_wide("GridPrint A4 Job").as_ptr()),
                lpszOutput: PCWSTR::null(),
                lpszDatatype: PCWSTR::null(),
                fwType: 0,
            };

            if StartDocW(hdc, &doc_info) <= 0 {
                return Err("Nepodařilo se spustit tiskovou úlohu.".to_string());
            }
            
            let pages = if copies_set_in_devmode { 1 } else { copies };

            let rgb_img = img.to_rgba8();
            let img_width = rgb_img.width();
            let img_height = rgb_img.height();
            let pixels = rgb_img.as_raw();

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: img_width as i32,
                    biHeight: -(img_height as i32), // Top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            for _ in 0..pages {
                StartPage(hdc);
                
                let printable_w = GetDeviceCaps(hdc, HORZRES);
                let printable_h = GetDeviceCaps(hdc, VERTRES);
                
                StretchDIBits(
                    hdc,
                    0, 0, printable_w, printable_h,
                    0, 0, img_width as i32, img_height as i32,
                    Some(pixels.as_ptr() as *const _),
                    &bmi,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
                
                EndPage(hdc);
            }

            EndDoc(hdc);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod win_print {
    use image::DynamicImage;
    pub fn get_default_printer() -> Option<String> { None }
    pub fn list_printers() -> Vec<String> {
        vec!["(Tisk nepodporován - uložte do PDF)".to_string()]
    }
    pub fn print_image(_img: &DynamicImage, _printer_name: &str, _copies: usize, _landscape: bool) -> Result<(), String> {
        Err("Tisk na tomto systému není podporován. Použijte možnost uložení do PDF.".to_string())
    }
}

// Main application state
struct GridPrintApp {
    image_path: Option<PathBuf>,
    loaded_image: Option<DynamicImage>,
    preview_base_image: Option<DynamicImage>, // Optimized downscaled base image for fast preview rendering
    image_info: String,
    grid_size: GridSize,
    orientation: Orientation,
    fill_mode: FillMode,
    copies: usize,
    printers: Vec<String>,
    selected_printer: String,
    preview_texture: Option<egui::TextureHandle>,
    preview_dirty: bool,
    status_message: Option<(String, bool)>, // (message, is_success)
    dark_mode: bool,
}

impl Default for GridPrintApp {
    fn default() -> Self {
        let printers = win_print::list_printers();
        let selected_printer = printers.first().cloned().unwrap_or_default();
        Self {
            image_path: None,
            loaded_image: None,
            preview_base_image: None,
            image_info: "Není vybrán žádný obrázek".to_string(),
            grid_size: GridSize::Grid2x2,
            orientation: Orientation::Portrait,
            fill_mode: FillMode::CropAndFill,
            copies: 1,
            printers,
            selected_printer,
            preview_texture: None,
            preview_dirty: false,
            status_message: None,
            dark_mode: true,
        }
    }
}

impl GridPrintApp {
    fn load_selected_image(&mut self, path: PathBuf) {
        let load_res = || -> Result<DynamicImage, image::ImageError> {
            image::io::Reader::open(&path)?.decode()
        }();
        match load_res {
            Ok(img) => {
                self.image_info = format!(
                    "Soubor: {}\nRozlišení: {}x{} px",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    img.width(),
                    img.height()
                );
                
                // Smart auto-orientation
                if img.width() > img.height() {
                    self.orientation = Orientation::Landscape;
                } else {
                    self.orientation = Orientation::Portrait;
                }
                
                // Optimized downscaling: scale the image down once to speed up preview grid generation
                let preview_base = img.resize(1000, 1000, image::imageops::FilterType::Triangle);
                self.preview_base_image = Some(preview_base);
                
                self.loaded_image = Some(img);
                self.image_path = Some(path);
                self.preview_dirty = true;
                self.status_message = None;
            }
            Err(e) => {
                self.status_message = Some((format!("Chyba při načítání obrázku: {}", e), false));
            }
        }
    }

    fn generate_composite(&self, width: u32, height: u32, use_preview_base: bool) -> Option<DynamicImage> {
        let (rows, cols) = self.grid_size.to_dims();
        if use_preview_base {
            let base = self.preview_base_image.as_ref()?;
            Some(generate_grid_image(base, rows, cols, width, height, self.fill_mode, image::imageops::FilterType::Triangle))
        } else {
            let base = self.loaded_image.as_ref()?;
            Some(generate_grid_image(base, rows, cols, width, height, self.fill_mode, image::imageops::FilterType::Lanczos3))
        }
    }

    fn update_preview_texture(&mut self, ctx: &egui::Context) {
        if !self.preview_dirty {
            return;
        }
        
        if self.loaded_image.is_some() {
            // Generate a medium resolution preview image (e.g. 600x848 or 848x600)
            let is_landscape = self.orientation == Orientation::Landscape;
            let (preview_w, preview_h) = if is_landscape { (848, 600) } else { (600, 848) };
            
            if let Some(composite) = self.generate_composite(preview_w, preview_h, true) {
                let rgba = composite.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();
                
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                self.preview_texture = Some(ctx.load_texture("preview", color_img, Default::default()));
                self.preview_dirty = false;
            }
        } else {
            self.preview_texture = None;
            self.preview_dirty = false;
        }
    }

    fn trigger_print(&mut self) {
        let copies = self.copies;
        let printer = self.selected_printer.clone();
        let landscape = self.orientation == Orientation::Landscape;
        
        // Print resolution (300 DPI)
        let (w, h) = if landscape { (3508, 2480) } else { (2480, 3508) };
        
        if let Some(print_img) = self.generate_composite(w, h, false) {
            match win_print::print_image(&print_img, &printer, copies, landscape) {
                Ok(_) => {
                    self.status_message = Some((
                        format!("Tisk odeslán na '{}' (kopií: {}).", printer, copies),
                        true
                    ));
                }
                Err(e) => {
                    self.status_message = Some((format!("Tisk selhal: {}", e), false));
                }
            }
        } else {
            self.status_message = Some(("Vyberte nejprve obrázek k tisku.".to_string(), false));
        }
    }

    fn trigger_save_pdf(&mut self, path: PathBuf) {
        let landscape = self.orientation == Orientation::Landscape;
        let (w, h) = if landscape { (3508, 2480) } else { (2480, 3508) };
        
        if let Some(output_img) = self.generate_composite(w, h, false) {
            match save_grid_pdf(&output_img, &path, landscape) {
                Ok(_) => {
                    self.status_message = Some((
                        format!("PDF úspěšně uloženo do: {}", path.file_name().unwrap_or_default().to_string_lossy()),
                        true
                    ));
                }
                Err(e) => {
                    self.status_message = Some((format!("Uložení PDF selhalo: {}", e), false));
                }
            }
        }
    }

    fn trigger_save_image(&mut self, path: PathBuf) {
        let landscape = self.orientation == Orientation::Landscape;
        let (w, h) = if landscape { (3508, 2480) } else { (2480, 3508) };
        
        if let Some(output_img) = self.generate_composite(w, h, false) {
            match output_img.save(&path) {
                Ok(_) => {
                    self.status_message = Some((
                        format!("Obrázek úspěšně uložen do: {}", path.file_name().unwrap_or_default().to_string_lossy()),
                        true
                    ));
                }
                Err(e) => {
                    self.status_message = Some((format!("Uložení obrázku selhalo: {}", e), false));
                }
            }
        }
    }
}

// Main Eframe Update loop
impl eframe::App for GridPrintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set appearance mode
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        self.update_preview_texture(ctx);

        // Sidebar Panel
        egui::SidePanel::left("control_panel")
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(20.0);
                    ui.heading("GridPrint A4");
                    ui.label("Tisk více kopií obrázku na A4");
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);
                });

                // Scrollable control elements
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // 1. File Selection
                        if ui.add(egui::Button::new("📁 Vybrat obrázek...").min_size(egui::vec2(280.0, 36.0))).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Obrázky", &["jpg", "jpeg", "png", "webp", "bmp"])
                                .pick_file() {
                                self.load_selected_image(path);
                            }
                        }
                        
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label(&self.image_info);
                        });
                        ui.add_space(15.0);

                        // 2. Grid layout
                        ui.label("Rozložení mřížky:");
                        ui.horizontal(|ui| {
                            for size in [GridSize::Grid2x2, GridSize::Grid3x3, GridSize::Grid4x4] {
                                if ui.selectable_value(&mut self.grid_size, size, size.label()).clicked() {
                                    self.preview_dirty = true;
                                }
                            }
                        });
                        ui.add_space(15.0);

                        // 3. Orientation
                        ui.label("Orientace stránky (A4):");
                        ui.horizontal(|ui| {
                            if ui.selectable_value(&mut self.orientation, Orientation::Portrait, "Na výšku").clicked() {
                                self.preview_dirty = true;
                            }
                            if ui.selectable_value(&mut self.orientation, Orientation::Landscape, "Na šířku").clicked() {
                                self.preview_dirty = true;
                            }
                        });
                        ui.add_space(15.0);

                        // 4. Fill mode
                        ui.label("Styl přizpůsobení buněk:");
                        ui.horizontal(|ui| {
                            if ui.selectable_value(&mut self.fill_mode, FillMode::CropAndFill, "Vyplnit").clicked() {
                                self.preview_dirty = true;
                            }
                            if ui.selectable_value(&mut self.fill_mode, FillMode::Fit, "Celý obrázek").clicked() {
                                self.preview_dirty = true;
                            }
                        });
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(15.0);

                        // 5. Copies (Spinbox)
                        ui.label("Počet kopií (stránek A4):");
                        ui.horizontal(|ui| {
                            if ui.button("-").clicked() {
                                if self.copies > 1 {
                                    self.copies -= 1;
                                }
                            }
                            let mut copies_str = self.copies.to_string();
                            let text_edit = ui.add_sized([60.0, 24.0], egui::TextEdit::singleline(&mut copies_str));
                            if text_edit.changed() {
                                if let Ok(val) = copies_str.parse::<usize>() {
                                    self.copies = val.max(1);
                                }
                            }
                            
                            if ui.button("+").clicked() {
                                self.copies += 1;
                            }
                        });
                        ui.add_space(15.0);

                        // 6. Printer selection
                        ui.label("Tiskárna:");
                        egui::ComboBox::from_id_source("printer_combo")
                            .width(280.0)
                            .selected_text(&self.selected_printer)
                            .show_ui(ui, |ui| {
                                for printer in &self.printers {
                                    ui.selectable_value(&mut self.selected_printer, printer.clone(), printer);
                                }
                            });
                        ui.add_space(20.0);

                        // 7. Print action
                        let print_btn = egui::Button::new("🖨️ Vytisknout")
                            .min_size(egui::vec2(280.0, 44.0))
                            .fill(egui::Color32::from_rgb(46, 204, 113));
                        
                        if ui.add(print_btn).clicked() {
                            self.trigger_print();
                        }
                        ui.add_space(10.0);

                        // 8. Save action
                        if ui.add(egui::Button::new("💾 Uložit jako PDF / Obrázek...").min_size(egui::vec2(280.0, 32.0))).clicked() {
                            if self.loaded_image.is_some() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PDF dokument", &["pdf"])
                                    .add_filter("PNG obrázek", &["png"])
                                    .add_filter("JPEG obrázek", &["jpg", "jpeg"])
                                    .save_file() 
                                {
                                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                        if ext.eq_ignore_ascii_case("pdf") {
                                            self.trigger_save_pdf(path);
                                        } else {
                                            self.trigger_save_image(path);
                                        }
                                    } else {
                                        // Default to PDF if no extension is typed
                                        let path_with_ext = path.with_extension("pdf");
                                        self.trigger_save_pdf(path_with_ext);
                                    }
                                }
                            } else {
                                self.status_message = Some(("Vyberte nejprve obrázek.".to_string(), false));
                            }
                        }
                        
                        ui.add_space(30.0);

                        // Dark mode toggle at the bottom of scroll area
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.dark_mode, "Tmavý režim");
                        });
                    });
            });

        // Central Panel (Preview Area) with custom contrasting background
        let panel_fill = if self.dark_mode {
            egui::Color32::from_gray(14)
        } else {
            egui::Color32::from_gray(215)
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(panel_fill))
            .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Náhled tisku na A4 (bez okrajů)");
                ui.add_space(5.0);

                // Show status notification if any
                if let Some((msg, is_success)) = &self.status_message {
                    let bg = if *is_success {
                        egui::Color32::from_rgba_unmultiplied(46, 204, 113, 30)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(231, 76, 60, 30)
                    };
                    let stroke_color = if *is_success {
                        egui::Color32::from_rgb(46, 204, 113)
                    } else {
                        egui::Color32::from_rgb(231, 76, 60)
                    };
                    
                    egui::Frame::none()
                        .fill(bg)
                        .stroke(egui::Stroke::new(1.0, stroke_color))
                        .rounding(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(msg);
                            });
                        });
                    ui.add_space(10.0);
                }

                // Main preview viewport area
                let avail_size = ui.available_size() - egui::vec2(20.0, 20.0);
                
                let is_landscape = self.orientation == Orientation::Landscape;
                let a4_ratio = if is_landscape { 1.4142 } else { 0.7071 };
                
                // Calculate A4 size that fits in viewport
                let (w, h) = if avail_size.x / avail_size.y > a4_ratio {
                    let h = avail_size.y;
                    (h * a4_ratio, h)
                } else {
                    let w = avail_size.x;
                    (w, w / a4_ratio)
                };

                ui.vertical_centered(|ui| {
                    if let Some(texture) = &self.preview_texture {
                        // Display A4 image inside a shadowed frame
                        egui::Frame::canvas(ui.style())
                            .fill(egui::Color32::WHITE)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(180)))
                            .shadow(egui::epaint::Shadow {
                                offset: egui::vec2(4.0, 4.0),
                                blur: 10.0,
                                spread: 0.0,
                                color: if self.dark_mode { egui::Color32::from_black_alpha(120) } else { egui::Color32::from_black_alpha(50) },
                            })
                            .inner_margin(0.0)
                            .show(ui, |ui| {
                                // Draw A4 grid image
                                let rect = ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(w, h))).rect;
                                // Draw a distinct thin border on top of the image to clearly delineate A4 page borders
                                ui.painter().rect_stroke(
                                    rect,
                                    0.0,
                                    egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
                                );
                            });
                    } else {
                        // Placeholder state (No image selected)
                        let placeholder_frame = egui::Frame::canvas(ui.style())
                            .fill(if self.dark_mode { egui::Color32::from_gray(24) } else { egui::Color32::from_gray(240) })
                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_gray(100)))
                            .rounding(4.0)
                            .shadow(egui::epaint::Shadow {
                                offset: egui::vec2(2.0, 2.0),
                                blur: 8.0,
                                spread: 0.0,
                                color: egui::Color32::from_black_alpha(50),
                            })
                            .inner_margin(20.0);

                        placeholder_frame.show(ui, |ui| {
                            ui.allocate_ui(egui::vec2(w, h), |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(h / 2.0 - 45.0);
                                    ui.label(egui::RichText::new("🖼️").size(48.0).color(egui::Color32::GRAY));
                                    ui.add_space(10.0);
                                    ui.label(egui::RichText::new("Kliknutím sem nebo tlačítkem vlevo\nvyberte obrázek k tisku.")
                                        .size(14.0)
                                        .strong()
                                        .color(egui::Color32::GRAY));
                                    
                                    // Clicking the placeholder canvas selects an image
                                    let rect = ui.max_rect();
                                    let response = ui.interact(rect, ui.id(), egui::Sense::click());
                                    if response.clicked() {
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter("Obrázky", &["jpg", "jpeg", "png", "webp", "bmp"])
                                            .pick_file() {
                                            self.load_selected_image(path);
                                        }
                                    }
                                });
                            });
                        });
                    }
                });
            });
        });
    }
}

// Composite grid image generator
fn generate_grid_image(
    base_image: &DynamicImage,
    grid_rows: usize,
    grid_cols: usize,
    width: u32,
    height: u32,
    fill_mode: FillMode,
    filter: image::imageops::FilterType,
) -> DynamicImage {
    let mut page = image::ImageBuffer::from_pixel(width, height, image::Rgba([255, 255, 255, 255]));

    let cell_w = width as f64 / grid_cols as f64;
    let cell_h = height as f64 / grid_rows as f64;

    for r in 0..grid_rows {
        for c in 0..grid_cols {
            let x1 = (c as f64 * cell_w) as u32;
            let y1 = (r as f64 * cell_h) as u32;
            let x2 = ((c + 1) as f64 * cell_w) as u32;
            let y2 = ((r + 1) as f64 * cell_h) as u32;

            let w = x2 - x1;
            let h = y2 - y1;

            if w == 0 || h == 0 {
                continue;
            }

            let img_w = base_image.width() as f64;
            let img_h = base_image.height() as f64;

            let resized = match fill_mode {
                FillMode::CropAndFill => {
                    let cell_aspect = w as f64 / h as f64;
                    let img_aspect = img_w / img_h;

                    if img_aspect > cell_aspect {
                        let new_h = h;
                        let new_w = (h as f64 * img_aspect) as u32;
                        let mut temp = base_image.resize_exact(new_w, new_h, filter);
                        let crop_x = (new_w - w) / 2;
                        temp.crop(crop_x, 0, w, h)
                    } else {
                        let new_w = w;
                        let new_h = (w as f64 / img_aspect) as u32;
                        let mut temp = base_image.resize_exact(new_w, new_h, filter);
                        let crop_y = (new_h - h) / 2;
                        temp.crop(0, crop_y, w, h)
                    }
                }
                FillMode::Fit => {
                    let cell_aspect = w as f64 / h as f64;
                    let img_aspect = img_w / img_h;

                    if img_aspect > cell_aspect {
                        let new_w = w;
                        let new_h = (w as f64 / img_aspect) as u32;
                        let temp = base_image.resize(new_w, new_h, filter);
                        
                        let mut cell_bg = image::ImageBuffer::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
                        let pad_y = (h - new_h) / 2;
                        image::imageops::overlay(&mut cell_bg, &temp.to_rgba8(), 0, pad_y as i64);
                        DynamicImage::ImageRgba8(cell_bg)
                    } else {
                        let new_h = h;
                        let new_w = (h as f64 * img_aspect) as u32;
                        let temp = base_image.resize(new_w, new_h, filter);
                        
                        let mut cell_bg = image::ImageBuffer::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
                        let pad_x = (w - new_w) / 2;
                        image::imageops::overlay(&mut cell_bg, &temp.to_rgba8(), pad_x as i64, 0);
                        DynamicImage::ImageRgba8(cell_bg)
                    }
                }
            };

            // Overlay cell into grid canvas
            image::imageops::overlay(&mut page, &resized.to_rgba8(), x1 as i64, y1 as i64);
        }
    }

    DynamicImage::ImageRgba8(page)
}

// PDF Exporter
fn save_grid_pdf(img: &DynamicImage, path: &Path, landscape: bool) -> Result<(), Box<dyn std::error::Error>> {
    use printpdf::*;
    use std::fs::File;
    use std::io::BufWriter;

    let (page_w, page_h) = if landscape {
        (Mm(297.0), Mm(210.0))
    } else {
        (Mm(210.0), Mm(297.0))
    };

    let (doc, page1, layer1) = PdfDocument::new("GridPrint PDF", page_w, page_h, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let print_img = Image::from_dynamic_image(img);

    // Calculate scale factor in points (1 mm = 2.8346 points)
    let w_pts = if landscape { 297.0 * 2.834645 } else { 210.0 * 2.834645 };
    let h_pts = if landscape { 210.0 * 2.834645 } else { 297.0 * 2.834645 };

    let img_w = img.width() as f64;
    let img_h = img.height() as f64;

    let scale_x = w_pts / img_w;
    let scale_y = h_pts / img_h;

    print_img.add_to_layer(
        current_layer,
        ImageTransform {
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            scale_x: Some(scale_x as f32),
            scale_y: Some(scale_y as f32),
            ..Default::default()
        },
    );

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    doc.save(&mut writer)?;
    Ok(())
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GridPrint A4 - Tisk mřížky obrázků")
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([950.0, 680.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "gridprint_app",
        options,
        Box::new(|_cc| Box::new(GridPrintApp::default())),
    )
}
