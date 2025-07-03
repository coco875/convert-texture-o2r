use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::Read,
};
use walkdir::WalkDir;
use zip::{self};

fn scale_3_8(value: u8) -> u8 {
    // Scale a 3-bit value to 8 bits
    (value as u16 * 255 / 7) as u8
}

fn scale_4_8(value: u8) -> u8 {
    // Scale a 4-bit value to 8 bits
    (value as u16 * 255 / 15) as u8
}

fn scale_5_8(value: u8) -> u8 {
    // Scale a 5-bit value to 8 bits
    (value as u16 * 255 / 31) as u8
}


#[derive(Debug, PartialEq)]
enum TextureType {
    Error,
    RGBA32bpp,
    RGBA16bpp,
    Palette4bpp,
    Palette8bpp,
    Grayscale4bpp,
    Grayscale8bpp,
    GrayscaleAlpha4bpp,
    GrayscaleAlpha8bpp,
    GrayscaleAlpha16bpp,
    GrayscaleAlpha1bpp,
    TLUT,
}

impl TextureType {
    fn from_u32(value: u32) -> Self {
        match value {
            0 => TextureType::Error,
            1 => TextureType::RGBA32bpp,
            2 => TextureType::RGBA16bpp,
            3 => TextureType::Palette4bpp,
            4 => TextureType::Palette8bpp,
            5 => TextureType::Grayscale4bpp,
            6 => TextureType::Grayscale8bpp,
            7 => TextureType::GrayscaleAlpha4bpp,
            8 => TextureType::GrayscaleAlpha8bpp,
            9 => TextureType::GrayscaleAlpha16bpp,
            10 => TextureType::GrayscaleAlpha1bpp,
            11 => TextureType::TLUT,
            _ => panic!("Unknown texture type ID"),
        }
    }

    fn to_image_type(&self) -> image::ExtendedColorType {
        match self {
            TextureType::RGBA32bpp => image::ExtendedColorType::Rgba8,
            TextureType::RGBA16bpp => image::ExtendedColorType::Rgba8,
            TextureType::Palette4bpp => image::ExtendedColorType::Rgba8,
            TextureType::Palette8bpp => image::ExtendedColorType::Rgba8,
            TextureType::Grayscale4bpp => image::ExtendedColorType::La8,
            TextureType::Grayscale8bpp => image::ExtendedColorType::La8,
            TextureType::GrayscaleAlpha4bpp => image::ExtendedColorType::La8,
            TextureType::GrayscaleAlpha8bpp => image::ExtendedColorType::La8,
            TextureType::GrayscaleAlpha16bpp => image::ExtendedColorType::La8,
            TextureType::GrayscaleAlpha1bpp => image::ExtendedColorType::La1,
            _ => panic!("Unsupported texture type for conversion to image type"),
        }
    }

    fn bits_per_pixel(&self) -> u8 {
        match self {
            TextureType::RGBA32bpp => 32,
            TextureType::RGBA16bpp => 16,
            TextureType::Palette4bpp => 4,
            TextureType::Palette8bpp => 8,
            TextureType::Grayscale4bpp => 4,
            TextureType::Grayscale8bpp => 8,
            TextureType::GrayscaleAlpha4bpp => 4,
            TextureType::GrayscaleAlpha8bpp => 8,
            TextureType::GrayscaleAlpha16bpp => 16,
            TextureType::GrayscaleAlpha1bpp => 1,
            _ => panic!("Unsupported texture type for bits per pixel"),
        }
    }
}

#[derive(Debug, PartialEq)]
enum ResourceType {
    None = 0x00000000,

    DisplayList = 0x4F444C54, // ODLT
    Light = 0x46669697,       // LGTS
    Matrix = 0x4F4D5458,      // OMTX
    Texture = 0x4F544558,     // OTEX
    Vertex = 0x4F565458,      // OVTX
}

const OTR_HEADER_SIZE: usize = 64;

struct OTRHeader {
    byte_order: i8,
    is_custom: bool,
    type_id: ResourceType,
    version: u32,
    id: u64,
}

impl OTRHeader {
    fn new(byte_order: i8, is_custom: bool, type_id: ResourceType, version: u32, id: u64) -> Self {
        OTRHeader {
            byte_order,
            is_custom,
            type_id,
            version,
            id,
        }
    }

    fn parse(data: &[u8]) -> Self {
        if data.len() < 20 {
            panic!("Data too short to parse OTR header");
        }
        let byte_order = data[0] as i8;
        let is_custom = data[1] != 0;
        let type_id = match u32::from_le_bytes([data[4], data[5], data[6], data[7]]) {
            0x00000000 => ResourceType::None,
            0x4F444C54 => ResourceType::DisplayList, // ODLT
            0x46669697 => ResourceType::Light,       // LGTS
            0x4F4D5458 => ResourceType::Matrix,      // OMTX
            0x4F544558 => ResourceType::Texture,     // OTEX
            0x4F565458 => ResourceType::Vertex,      // OVTX
            _ => ResourceType::None,
        };
        let version = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let id = u64::from_le_bytes([
            data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
        ]);
        OTRHeader::new(byte_order, is_custom, type_id, version, id)
    }
}

struct TextureFormat {
    type_id: TextureType,
    width: u32,
    height: u32,
    size: u32,
    data: Vec<u8>,
}

impl TextureFormat {
    fn new(type_id: TextureType, width: u32, height: u32, size: u32, data: Vec<u8>) -> Self {
        TextureFormat {
            type_id,
            width,
            height,
            size,
            data,
        }
    }

    fn parse(data: &[u8]) -> Self {
        if data.len() < 24 {
            panic!("Data too short to parse texture format");
        }
        let type_id = match u32::from_le_bytes([
            data[OTR_HEADER_SIZE],
            data[OTR_HEADER_SIZE + 1],
            data[OTR_HEADER_SIZE + 2],
            data[OTR_HEADER_SIZE + 3],
        ]) {
            0 => TextureType::Error,
            1 => TextureType::RGBA32bpp,
            2 => TextureType::RGBA16bpp,
            3 => TextureType::Palette4bpp,
            4 => TextureType::Palette8bpp,
            5 => TextureType::Grayscale4bpp,
            6 => TextureType::Grayscale8bpp,
            7 => TextureType::GrayscaleAlpha4bpp,
            8 => TextureType::GrayscaleAlpha8bpp,
            9 => TextureType::GrayscaleAlpha16bpp,
            10 => TextureType::GrayscaleAlpha1bpp,
            11 => TextureType::TLUT,
            _ => panic!("Unknown texture type ID"),
        };
        let width = u32::from_le_bytes([
            data[OTR_HEADER_SIZE + 4],
            data[OTR_HEADER_SIZE + 5],
            data[OTR_HEADER_SIZE + 6],
            data[OTR_HEADER_SIZE + 7],
        ]);
        let height = u32::from_le_bytes([
            data[OTR_HEADER_SIZE + 8],
            data[OTR_HEADER_SIZE + 9],
            data[OTR_HEADER_SIZE + 10],
            data[OTR_HEADER_SIZE + 11],
        ]);
        let size = u32::from_le_bytes([
            data[OTR_HEADER_SIZE + 12],
            data[OTR_HEADER_SIZE + 13],
            data[OTR_HEADER_SIZE + 14],
            data[OTR_HEADER_SIZE + 15],
        ]);
        let texture_data = data[OTR_HEADER_SIZE + 16..].to_vec();

        TextureFormat::new(type_id, width, height, size, texture_data)
    }
}

fn convert_texture(data: Vec<u8>) {
    let otr_format = OTRHeader::parse(&data);
    let texture_format = TextureFormat::parse(&data);

    println!("byte_order: {}", otr_format.byte_order);
    println!("is_custom: {}", otr_format.is_custom);
    println!("version: {}", otr_format.version);
    println!("id: {}", otr_format.id);

    println!("type_id: {:?}", texture_format.type_id);
    println!("width: {}", texture_format.width);
    println!("height: {}", texture_format.height);
    println!("size: {}", texture_format.size);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);
    let zip_file = args
        .get(1)
        .expect("Please provide a zip file path as the first argument.");
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(zip_file).expect("Failed to open zip file"))
            .expect("Failed to read zip file");
    println!("Number of files in zip: {}", zip.len());

    let config_file = "config.yml";
    if !std::path::Path::new(config_file).exists() {
        panic!("Configuration file '{}' not found.", config_file);
    }

    let config = yaml_rust2::YamlLoader::load_from_str(
        &std::fs::read_to_string(config_file).expect("Failed to read config file"),
    )
    .expect("Failed to parse YAML config file");

    let mut tlut_texture: HashSet<String> = HashSet::new();
    let mut texture_tlut: HashMap<String, String> = HashMap::new();
    let mut texture_palette: HashMap<String, TextureFormat> = HashMap::new();

    let config = &config[0];

    // get the first element of the hashmap
    let mut path = "";
    let key_path = yaml_rust2::Yaml::String("path".to_owned());
    for (_, value) in config.as_hash().expect("Config is not a hash") {
        let hash_map = value.as_hash().unwrap();
        if hash_map.contains_key(&key_path) {
            path = hash_map
                .get(&key_path)
                .expect("Path key not found in config")
                .as_str()
                .expect("Path value is not a string");
            break;
        }
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|file| file.ok())
        .filter(|file| file.file_type().is_file())
        .map(|file| {
            file.path()
                .to_str()
                .expect("Failed to convert path to string")
                .to_owned()
        })
        .filter(|file| file.ends_with(".yml") || file.ends_with(".yaml"))
        .filter_map(|file_path| {
            yaml_rust2::YamlLoader::load_from_str(&std::fs::read_to_string(file_path).ok()?).ok()
        })
        .flat_map(std::convert::identity)
        .filter_map(|yaml| yaml.into_hash())
        .flat_map(std::convert::identity)
        .filter_map(|(key, value)| {
            let object = value.as_hash()?;
            let mut tlut = { object.get(&yaml_rust2::Yaml::String("tlut".to_owned())) };
            if tlut.is_none() {
                tlut = object.get(&yaml_rust2::Yaml::String("tlut_symbol".to_owned()));
            }
            let tlut = tlut?;
            let tlut_str = tlut.as_str()?;
            Some((key, tlut_str.to_owned()))
        })
        .for_each(|(key, tlut_str)| {
            tlut_texture.insert(tlut_str.to_owned());
            texture_tlut.insert(
                key.as_str().expect("Key is not a string").to_owned(),
                tlut_str.to_owned(),
            );
        });

    let file_names = zip
        .file_names()
        .map(|name| name.to_owned())
        .collect::<Vec<String>>();

    for path in file_names.clone().into_iter().filter(|path| {
        tlut_texture
            .iter()
            .filter(|tlut| path.contains(*tlut))
            .count()
            > 0
    }) {
        let Some(mut file) = zip.by_name(&path).ok() else {
            continue;
        };
        let mut data = Vec::new();
        let _ = file.read_to_end(&mut data);
        texture_palette.insert(file.name().to_owned(), TextureFormat::parse(&data));
    }

    let folder_name = "textures";
    fs::remove_dir_all(folder_name).ok();
    fs::create_dir_all(folder_name).expect("Failed to create folder");

    println!("{:?} TLUT textures found", texture_tlut);

    for path in file_names {
        let Some(mut file) = zip.by_name(&path).ok() else {
            continue;
        };
        let mut data = Vec::new();
        let _ = file.read_to_end(&mut data);
        if data.len() < OTR_HEADER_SIZE {
            println!("File {} is too short to be a valid OTR file", file.name());
            continue;
        }
        let otr_format = OTRHeader::parse(&data);
        if otr_format.type_id != ResourceType::Texture {
            continue;
        }
        let texture_format = TextureFormat::parse(&data);
        let name = file.name().to_owned();
        if !(otr_format.type_id == ResourceType::Texture
            && texture_format.type_id != TextureType::Error
            && texture_format.type_id != TextureType::TLUT)
        {
            continue;
        }

        let current_folder_name = name.split('/').next().unwrap();

        let path = folder_name.to_owned() + "/" + &name + ".png";
        let file_name = name.split('/').last().unwrap();

        println!("Processing texture: {}", path);

        let _ = fs::create_dir(folder_name.to_owned() + "/" + current_folder_name);

        let format = texture_format.type_id.to_image_type();
        let mut data = texture_format.data;

        println!("size: {}", texture_format.size);

        if (((texture_format.type_id.bits_per_pixel() as u32 * texture_format.width * texture_format.height)/8) as usize)
            > data.len()
        {
            println!(
                "Data size does not match expected size for {}: {} vs {}",
                file.name(),
                data.len(),
                ((format.bits_per_pixel() as u32 * texture_format.width * texture_format.height)/8) as usize
            );
            continue;
        }

        match texture_format.type_id {
            TextureType::RGBA32bpp => {
                println!("Converting RGBA32bpp texture");
            }
            TextureType::RGBA16bpp => {
                println!("Converting RGBA16bpp texture");
                let mut new_data = Vec::with_capacity(
                    (texture_format.height * texture_format.width * 4)
                        .try_into()
                        .unwrap(),
                );
                for i in 0..(texture_format.height * texture_format.width) as usize {
                    new_data.push(scale_5_8((data[i * 2] & 0xF8) >> 3)); // R
                    new_data.push(scale_5_8(((data[i * 2] & 0x07) << 2) | ((data[i * 2 + 1] & 0xc0) >> 6))); // G
                    new_data.push(scale_5_8((data[i * 2 + 1] & 0x3E) >> 1)); // B
                    new_data.push(if (data[i * 2 + 1] & 0x01) != 0 { 0xFF } else { 0x00 }); // A
                }
                data = new_data;
            }
            TextureType::Palette4bpp => {
                println!("Converting Palette4bpp texture");
            }
            TextureType::Palette8bpp => {
                println!("Converting Palette8bpp texture");
                let mut new_data = Vec::with_capacity(
                    (texture_format.height * texture_format.width * 4)
                        .try_into()
                        .unwrap(),
                );
                if !texture_tlut.contains_key(file_name) {
                    println!("Texture TLUT not found for {}", file_name);
                    continue;
                }

                let tlut = texture_tlut.get(file_name).unwrap();
                let Some(tlut) = texture_palette
                        .iter().find(|(name, _)| name.contains(tlut)) else {
                    println!("Texture TLUT not found for {}", file_name);
                    continue;
                };

                for i in 0..(texture_format.height * texture_format.width) as usize {
                    let index = data[i] as usize;
                    let color = tlut
                        .1
                        .data
                        .chunks(2)
                        .nth(index as usize)
                        .unwrap_or(&[1, 1]);
                    let r = scale_5_8((color[0] & 0xF8) >> 3);
                    let g = scale_5_8(((color[0] & 0x07) << 2) | ((color[1] & 0xc0) >> 6));
                    let b = scale_5_8((color[1] & 0x3E) >> 1);
                    let a = if (color[1] & 0x03) != 0 {
                        0xFF
                    } else {
                        0x00
                    };
                    new_data.push(r); // R
                    new_data.push(g); // G
                    new_data.push(b); // B
                    new_data.push(a); // A
                }
                data = new_data;
            }
            TextureType::Grayscale4bpp => {
                println!("Converting Grayscale4bpp texture");
                let mut new_data = Vec::with_capacity(
                    (texture_format.height * texture_format.width)
                        .try_into()
                        .unwrap(),
                );
                for i in 0..(texture_format.height * texture_format.width) as usize {
                    let mut bits = data[i / 2];
                    if i % 2 != 0 {
                        bits &= 0xF;
                    } else {
                        bits >>= 4;
                    }
                    new_data.push(scale_4_8(bits));
                    new_data.push(scale_4_8(bits));
                }
                data = new_data;
            }
            TextureType::Grayscale8bpp => {
                println!("Converting Grayscale8bpp texture");
                let mut new_data = Vec::with_capacity(
                    (texture_format.height * texture_format.width * 2)
                        .try_into()
                        .unwrap(),
                );
                for i in 0..(texture_format.height * texture_format.width) as usize {
                    let bits = data[i];
                    new_data.push(bits); // Grayscale
                    new_data.push(bits); // Alpha
                }
                data = new_data;
            }
            TextureType::GrayscaleAlpha4bpp => {
                println!("Converting GrayscaleAlpha4bpp texture");
                let mut new_data = Vec::with_capacity(
                    (texture_format.height * texture_format.width * 2)
                        .try_into()
                        .unwrap(),
                );
                for i in 0..(texture_format.height * texture_format.width) as usize {
                    let mut bits = data[i / 2];
                    if i % 2 != 0 {
                        bits &= 0xF;
                    } else {
                        bits >>= 4;
                    }
                    new_data.push(scale_3_8((bits >> 1) & 0x07));
                    new_data.push(if (bits & 0x01) != 0 { 0xFF } else { 0x00 });
                }
                data = new_data;
            }
            TextureType::GrayscaleAlpha8bpp => {
                println!("Converting GrayscaleAlpha8bpp texture");
                let mut new_data = Vec::with_capacity(
                    (texture_format.height * texture_format.width * 2)
                        .try_into()
                        .unwrap(),
                );
                for i in 0..(texture_format.height * texture_format.width) as usize {
                    let bits = data[i];
                    new_data.push(scale_4_8((bits & 0xF0) >> 4)); // Grayscale
                    new_data.push(scale_4_8(bits & 0x0F)); // Alpha
                }
                data = new_data;
            }
            TextureType::GrayscaleAlpha16bpp => {
                println!("Converting GrayscaleAlpha16bpp texture");
            }
            TextureType::GrayscaleAlpha1bpp => {
                println!("Converting GrayscaleAlpha1bpp texture");
            }
            _ => {
                println!(
                    "Unknown or unsupported texture type: {:?}",
                    texture_format.type_id
                );
                continue;
            }
        }

        image::save_buffer(
            path,
            &data,
            texture_format.width,
            texture_format.height,
            format,
        )
        .unwrap();
    }
}
