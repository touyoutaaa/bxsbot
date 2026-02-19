use anyhow::Result;
use lopdf::{Document, Object, ObjectId, Dictionary};
use tracing::{info, warn, debug};
use std::path::Path;
use std::io::Read as IoRead;

use super::ExtractedImage;

pub struct ImageAnalyzer;

impl ImageAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 从PDF中提取嵌入的图片
    pub fn extract_images(&self, pdf_path: &str, paper_id: &str, images_dir: &str) -> Result<Vec<ExtractedImage>> {
        info!("提取PDF图片: {}", pdf_path);

        if !Path::new(pdf_path).exists() {
            return Err(anyhow::anyhow!("PDF文件不存在: {}", pdf_path));
        }

        std::fs::create_dir_all(images_dir)?;

        let doc = Document::load(pdf_path)?;
        let mut images: Vec<ExtractedImage> = Vec::new();
        let mut img_index = 0;

        // Collect all image stream ObjectIds from the entire document
        // by scanning every object, rather than navigating the page tree
        let image_ids = self.collect_all_image_ids(&doc);
        info!("PDF中发现 {} 个Image对象", image_ids.len());

        for (obj_id, page_hint) in &image_ids {
            let obj = match doc.get_object(*obj_id) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let stream = match obj.as_stream() {
                Ok(s) => s,
                Err(_) => continue,
            };

            let width = stream.dict.get(b"Width")
                .ok()
                .and_then(|w| w.as_i64().ok())
                .unwrap_or(0) as u32;
            let height = stream.dict.get(b"Height")
                .ok()
                .and_then(|h| h.as_i64().ok())
                .unwrap_or(0) as u32;

            // Skip tiny images (likely icons/bullets, not figures)
            if width < 10 || height < 10 {
                debug!("跳过小图片: {}x{} (obj {:?})", width, height, obj_id);
                continue;
            }

            let filter_name = self.get_filter_name(&stream.dict);
            debug!("Image obj {:?}: {}x{}, filter={:?}, page~{}",
                obj_id, width, height, filter_name, page_hint);

            match filter_name.as_deref() {
                Some("DCTDecode") => {
                    // JPEG data
                    let data = stream.decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    if data.is_empty() {
                        warn!("JPEG数据为空 (obj {:?})", obj_id);
                        continue;
                    }
                    let filename = format!("{}/{}_img_{}.jpg", images_dir, paper_id, img_index);
                    if let Err(e) = std::fs::write(&filename, &data) {
                        warn!("写入JPEG失败: {}", e);
                        continue;
                    }
                    images.push(ExtractedImage {
                        filename,
                        page: *page_hint,
                        width,
                        height,
                        format: "jpeg".to_string(),
                    });
                    img_index += 1;
                }
                Some("FlateDecode") => {
                    // Try lopdf's decompressed_content first, fall back to manual flate2
                    let data = match stream.decompressed_content() {
                        Ok(d) => d,
                        Err(_) => {
                            // Manual decompression with flate2
                            match self.manual_inflate(&stream.content) {
                                Ok(d) => d,
                                Err(e) => {
                                    warn!("FlateDecode解压失败 (obj {:?}): {}", obj_id, e);
                                    continue;
                                }
                            }
                        }
                    };

                    let bits = stream.dict.get(b"BitsPerComponent")
                        .ok()
                        .and_then(|b| b.as_i64().ok())
                        .unwrap_or(8) as u32;

                    // Check if this is an Indexed (palette) color space
                    if let Some(rgb_data) = self.try_decode_indexed(&stream.dict, &doc, &data, width, height, bits) {
                        let filename = format!("{}/{}_img_{}.png", images_dir, paper_id, img_index);
                        let expected = (width * height * 3) as usize;
                        if rgb_data.len() >= expected {
                            if let Some(img) = image::RgbImage::from_raw(width, height, rgb_data[..expected].to_vec()) {
                                if let Err(e) = image::DynamicImage::ImageRgb8(img).save(&filename) {
                                    warn!("保存Indexed图片失败: {}", e);
                                    continue;
                                }
                                images.push(ExtractedImage {
                                    filename,
                                    page: *page_hint,
                                    width,
                                    height,
                                    format: "png".to_string(),
                                });
                                img_index += 1;
                                continue;
                            }
                        }
                    }

                    let channels = self.get_color_channels(&stream.dict, &doc);
                    let expected_size = (width * height * channels * bits / 8) as usize;

                    if data.len() < expected_size {
                        warn!("图片数据不匹配: {} < {} (obj {:?}, {}x{}, ch={}, bits={})",
                            data.len(), expected_size, obj_id, width, height, channels, bits);
                        continue;
                    }

                    let filename = format!("{}/{}_img_{}.png", images_dir, paper_id, img_index);
                    let img_result = match channels {
                        1 => image::GrayImage::from_raw(width, height, data[..expected_size].to_vec())
                            .map(image::DynamicImage::ImageLuma8),
                        3 => image::RgbImage::from_raw(width, height, data[..expected_size].to_vec())
                            .map(image::DynamicImage::ImageRgb8),
                        4 => image::RgbaImage::from_raw(width, height, data[..expected_size].to_vec())
                            .map(image::DynamicImage::ImageRgba8),
                        _ => {
                            warn!("不支持的通道数: {} (obj {:?})", channels, obj_id);
                            continue;
                        }
                    };

                    match img_result {
                        Some(img) => {
                            if let Err(e) = img.save(&filename) {
                                warn!("保存PNG失败: {}", e);
                                continue;
                            }
                            images.push(ExtractedImage {
                                filename,
                                page: *page_hint,
                                width,
                                height,
                                format: "png".to_string(),
                            });
                            img_index += 1;
                        }
                        None => {
                            warn!("无法创建图片 (obj {:?}, {}x{}, ch={})", obj_id, width, height, channels);
                        }
                    }
                }
                Some("JPXDecode") => {
                    let data = stream.decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    if data.is_empty() { continue; }
                    let filename = format!("{}/{}_img_{}.jp2", images_dir, paper_id, img_index);
                    if let Err(e) = std::fs::write(&filename, &data) {
                        warn!("写入JP2失败: {}", e);
                        continue;
                    }
                    images.push(ExtractedImage {
                        filename,
                        page: *page_hint,
                        width,
                        height,
                        format: "jp2".to_string(),
                    });
                    img_index += 1;
                }
                Some(other) => {
                    warn!("跳过不支持的编码: {} (obj {:?}, {}x{})", other, obj_id, width, height);
                }
                None => {
                    // Uncompressed raw data
                    let data = &stream.content;
                    if data.is_empty() { continue; }
                    let channels = self.get_color_channels(&stream.dict, &doc);
                    let bits = stream.dict.get(b"BitsPerComponent")
                        .ok()
                        .and_then(|b| b.as_i64().ok())
                        .unwrap_or(8) as u32;
                    let expected_size = (width * height * channels * bits / 8) as usize;
                    if data.len() < expected_size { continue; }
                    let filename = format!("{}/{}_img_{}.png", images_dir, paper_id, img_index);
                    let img_result = match channels {
                        1 => image::GrayImage::from_raw(width, height, data[..expected_size].to_vec())
                            .map(image::DynamicImage::ImageLuma8),
                        3 => image::RgbImage::from_raw(width, height, data[..expected_size].to_vec())
                            .map(image::DynamicImage::ImageRgb8),
                        _ => continue,
                    };
                    if let Some(img) = img_result {
                        if img.save(&filename).is_ok() {
                            images.push(ExtractedImage {
                                filename,
                                page: *page_hint,
                                width,
                                height,
                                format: "png".to_string(),
                            });
                            img_index += 1;
                        }
                    }
                }
            }
        }

        info!("图片提取完成，共 {} 张", images.len());
        Ok(images)
    }

    /// 遍历文档所有对象，找出 Subtype=Image 的 Stream 对象
    /// 这种方式不依赖页面树结构，能找到所有图片（包括嵌套在 Form XObject 中的）
    fn collect_all_image_ids(&self, doc: &Document) -> Vec<(ObjectId, usize)> {
        let mut image_ids: Vec<(ObjectId, usize)> = Vec::new();

        // Build a rough page mapping: which page references which objects
        // For simplicity, we just scan all objects directly
        for (&obj_id, object) in doc.objects.iter() {
            let stream = match object.as_stream() {
                Ok(s) => s,
                Err(_) => continue,
            };

            let subtype = stream.dict.get(b"Subtype")
                .ok()
                .and_then(|s| s.as_name().ok())
                .and_then(|n| std::str::from_utf8(n).ok());

            if subtype == Some("Image") {
                // Try to determine which page this belongs to (best effort)
                let page_num = self.find_page_for_object(doc, obj_id).unwrap_or(0);
                image_ids.push((obj_id, page_num));
            }
        }

        // Sort by page number for consistent ordering
        image_ids.sort_by_key(|(_, page)| *page);
        image_ids
    }

    /// Best-effort: find which page an object belongs to by checking page XObject references
    fn find_page_for_object(&self, doc: &Document, target_id: ObjectId) -> Option<usize> {
        for (page_num, page_id) in doc.get_pages() {
            if self.page_references_object(doc, page_id, target_id, 0) {
                return Some(page_num as usize);
            }
        }
        None
    }

    /// Check if a page (or its Form XObjects) references the target object, with depth limit
    fn page_references_object(&self, doc: &Document, start_id: ObjectId, target_id: ObjectId, depth: u32) -> bool {
        if depth > 3 { return false; }

        let obj = match doc.get_object(start_id) {
            Ok(o) => o,
            Err(_) => return false,
        };
        let dict = match obj.as_dict().or_else(|_| obj.as_stream().map(|s| &s.dict)) {
            Ok(d) => d,
            Err(_) => return false,
        };

        // Get Resources -> XObject dict
        let xobj_dict = match self.get_xobjects_from_dict(doc, dict) {
            Some(d) => d,
            None => return false,
        };

        for (_, val) in xobj_dict.iter() {
            if let Ok(ref_id) = val.as_reference() {
                if ref_id == target_id {
                    return true;
                }
                // Check if this is a Form XObject that might contain the target
                if let Ok(ref_obj) = doc.get_object(ref_id) {
                    if let Ok(ref_stream) = ref_obj.as_stream() {
                        let sub = ref_stream.dict.get(b"Subtype")
                            .ok()
                            .and_then(|s| s.as_name().ok())
                            .and_then(|n| std::str::from_utf8(n).ok());
                        if sub == Some("Form") {
                            if self.page_references_object(doc, ref_id, target_id, depth + 1) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Extract XObject dictionary from a dict's Resources
    fn get_xobjects_from_dict<'a>(&self, doc: &'a Document, dict: &'a Dictionary) -> Option<&'a Dictionary> {
        let res_ref = dict.get(b"Resources").ok()?;
        let (_, res_obj) = doc.dereference(res_ref).ok()?;
        let res_dict = res_obj.as_dict().ok()?;
        let xobj_ref = res_dict.get(b"XObject").ok()?;
        let (_, xobj_obj) = doc.dereference(xobj_ref).ok()?;
        xobj_obj.as_dict().ok()
    }

    /// 手动使用 flate2 解压数据（lopdf 的 decompressed_content 有时会失败）
    fn manual_inflate(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        // Try zlib (with header) first
        let mut decoder = flate2::read::ZlibDecoder::new(compressed);
        let mut result = Vec::new();
        match decoder.read_to_end(&mut result) {
            Ok(_) => return Ok(result),
            Err(_) => {}
        }

        // Fall back to raw deflate (no header)
        let mut decoder = flate2::read::DeflateDecoder::new(compressed);
        result.clear();
        decoder.read_to_end(&mut result)?;
        Ok(result)
    }

    /// 尝试解码 Indexed (调色板) 颜色空间的图片数据为 RGB
    /// Indexed 格式: [/Indexed base hival lookup_table]
    /// 每像素 1 字节索引值，通过 lookup_table 映射到 base 色彩空间 (通常 RGB)
    fn try_decode_indexed(&self, dict: &Dictionary, doc: &Document, data: &[u8], width: u32, height: u32, bits: u32) -> Option<Vec<u8>> {
        let cs_obj = dict.get(b"ColorSpace").ok()?;
        let arr = cs_obj.as_array().ok()?;

        // First element must be /Indexed
        let first_name = arr.first()?.as_name().ok()?;
        if std::str::from_utf8(first_name).ok()? != "Indexed" {
            return None;
        }

        // arr[1] = base color space (usually /DeviceRGB)
        // arr[2] = hival (max index, integer)
        // arr[3] = lookup table (string or stream)
        let hival = arr.get(2)?.as_i64().ok()? as usize;

        // Determine base color space channels
        let base_channels: usize = if let Some(base_cs) = arr.get(1) {
            if let Ok(name) = base_cs.as_name() {
                Self::channels_from_name(std::str::from_utf8(name).unwrap_or("DeviceRGB")) as usize
            } else {
                3
            }
        } else {
            3
        };

        // Get the lookup table bytes
        let lookup_data: Vec<u8> = if let Some(lookup_obj) = arr.get(3) {
            match lookup_obj {
                Object::String(bytes, _) => bytes.clone(),
                Object::Reference(ref_id) => {
                    if let Ok(obj) = doc.get_object(*ref_id) {
                        match obj {
                            Object::String(bytes, _) => bytes.clone(),
                            Object::Stream(stream) => {
                                stream.decompressed_content().unwrap_or_else(|_| stream.content.clone())
                            }
                            _ => return None,
                        }
                    } else {
                        return None;
                    }
                }
                Object::Stream(stream) => {
                    stream.decompressed_content().unwrap_or_else(|_| stream.content.clone())
                }
                _ => return None,
            }
        } else {
            return None;
        };

        let expected_lookup_size = (hival + 1) * base_channels;
        if lookup_data.len() < expected_lookup_size {
            debug!("Indexed lookup表太小: {} < {}", lookup_data.len(), expected_lookup_size);
            return None;
        }

        // Decode: each pixel byte is an index into the lookup table
        let pixel_count = (width * height) as usize;
        let bytes_per_row = ((width * bits + 7) / 8) as usize;
        let expected_data = bytes_per_row * height as usize;

        if data.len() < expected_data {
            debug!("Indexed图片数据不足: {} < {}", data.len(), expected_data);
            return None;
        }

        let mut rgb_data = Vec::with_capacity(pixel_count * base_channels);

        for i in 0..pixel_count {
            let idx = data[i] as usize;
            let idx = idx.min(hival);
            let offset = idx * base_channels;
            if offset + base_channels <= lookup_data.len() {
                rgb_data.extend_from_slice(&lookup_data[offset..offset + base_channels]);
            } else {
                // Fallback: black pixel
                rgb_data.extend(std::iter::repeat(0u8).take(base_channels));
            }
        }

        Some(rgb_data)
    }

    /// 获取 Filter 名称，处理 Name 和 Array 两种格式
    fn get_filter_name(&self, dict: &Dictionary) -> Option<String> {
        let filter_obj = dict.get(b"Filter").ok()?;

        // Try as Name first
        if let Ok(name_bytes) = filter_obj.as_name() {
            return std::str::from_utf8(name_bytes).ok().map(|s| s.to_string());
        }

        // Try as Array (e.g. [/FlateDecode] or [/ASCII85Decode /FlateDecode])
        if let Ok(arr) = filter_obj.as_array() {
            for item in arr.iter().rev() {
                if let Ok(name_bytes) = item.as_name() {
                    if let Ok(name) = std::str::from_utf8(name_bytes) {
                        if matches!(name, "DCTDecode" | "JPXDecode" | "CCITTFaxDecode") {
                            return Some(name.to_string());
                        }
                    }
                }
            }
            if let Some(first) = arr.first() {
                if let Ok(name_bytes) = first.as_name() {
                    return std::str::from_utf8(name_bytes).ok().map(|s| s.to_string());
                }
            }
        }

        None
    }

    /// 获取颜色通道数
    fn get_color_channels(&self, dict: &Dictionary, doc: &Document) -> u32 {
        let cs_obj = match dict.get(b"ColorSpace") {
            Ok(obj) => obj,
            Err(_) => return 3,
        };

        if let Ok(name_bytes) = cs_obj.as_name() {
            return Self::channels_from_name(std::str::from_utf8(name_bytes).unwrap_or(""));
        }

        if let Ok(arr) = cs_obj.as_array() {
            if let Some(first) = arr.first() {
                if let Ok(name_bytes) = first.as_name() {
                    let name = std::str::from_utf8(name_bytes).unwrap_or("");
                    match name {
                        "ICCBased" => {
                            if let Some(ref_obj) = arr.get(1) {
                                if let Ok(ref_id) = ref_obj.as_reference() {
                                    if let Ok(icc_obj) = doc.get_object(ref_id) {
                                        if let Ok(icc_stream) = icc_obj.as_stream() {
                                            if let Ok(n) = icc_stream.dict.get(b"N") {
                                                if let Ok(n_val) = n.as_i64() {
                                                    return n_val as u32;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            return 3;
                        }
                        "Indexed" | "CalRGB" | "Lab" => return 3,
                        "CalGray" => return 1,
                        "DeviceN" => {
                            if let Some(names) = arr.get(1) {
                                if let Ok(names_arr) = names.as_array() {
                                    return names_arr.len() as u32;
                                }
                            }
                            return 3;
                        }
                        _ => return Self::channels_from_name(name),
                    }
                }
            }
        }

        if let Ok(ref_id) = cs_obj.as_reference() {
            if let Ok(resolved) = doc.get_object(ref_id) {
                if let Ok(name_bytes) = resolved.as_name() {
                    return Self::channels_from_name(std::str::from_utf8(name_bytes).unwrap_or(""));
                }
            }
        }

        3
    }

    fn channels_from_name(name: &str) -> u32 {
        match name {
            "DeviceGray" | "CalGray" | "G" => 1,
            "DeviceRGB" | "CalRGB" | "RGB" => 3,
            "DeviceCMYK" | "CMYK" => 4,
            _ => 3,
        }
    }
}
