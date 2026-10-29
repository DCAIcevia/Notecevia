//! 比两张截图差了多少像素。用在界面改动的验收上：改前改后各存一组图，逐张比。
//!
//! 用法：
//! cargo run -p notecevia-app --example shot_diff -- <改前目录> <改后目录> [容差]
//!
//! 只认两边同名的文件；缺一边算失败。有差异就打出像素数与包围盒，并以非零码退出，
//! 这样它可以当门禁用，而不是靠人盯图。
//!
//! 容差是每通道允许的最大差值，默认 0：默认要求逐像素完全一致。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::RgbaImage;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() < 2 {
        eprintln!("用法：shot_diff <改前目录> <改后目录> [容差]");
        return ExitCode::FAILURE;
    }
    let before = PathBuf::from(&arguments[0]);
    let after = PathBuf::from(&arguments[1]);
    let tolerance: u8 = arguments
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    let names = match names_in(&before) {
        Ok(names) => names,
        Err(error) => {
            eprintln!("读不了 {}：{error}", before.display());
            return ExitCode::FAILURE;
        }
    };
    if names.is_empty() {
        eprintln!("{} 里没有图，先截一组", before.display());
        return ExitCode::FAILURE;
    }

    let mut failed = 0usize;
    for name in &names {
        let left = read(&before.join(name));
        let right = read(&after.join(name));
        let (left, right) = match (left, right) {
            (Ok(left), Ok(right)) => (left, right),
            (Err(error), _) | (_, Err(error)) => {
                println!("{name}: 读图失败 {error}");
                failed += 1;
                continue;
            }
        };
        if left.dimensions() != right.dimensions() {
            println!(
                "{name}: 尺寸变了 {:?} → {:?}",
                left.dimensions(),
                right.dimensions()
            );
            failed += 1;
            continue;
        }
        match difference(&left, &right, tolerance) {
            None => println!("{name}: 一致"),
            Some((count, box_)) => {
                let total = left.width() as u64 * left.height() as u64;
                println!(
                    "{name}: {count} 像素不同（{:.3}%），范围 {box_:?}",
                    count as f64 * 100.0 / total as f64
                );
                failed += 1;
            }
        }
    }
    if failed > 0 {
        println!("\n{failed} 张不一致");
        return ExitCode::FAILURE;
    }
    println!("\n{} 张全部一致", names.len());
    ExitCode::SUCCESS
}

fn names_in(directory: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".png") {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

fn read(path: &Path) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|image| image.to_rgba8())
        .map_err(|error| error.to_string())
}

/// 有差异就返回不同像素数与包围盒。
fn difference(
    left: &RgbaImage,
    right: &RgbaImage,
    tolerance: u8,
) -> Option<(u64, (u32, u32, u32, u32))> {
    let mut count = 0u64;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (x, y, pixel) in left.enumerate_pixels() {
        let other = right.get_pixel(x, y);
        let differs = (0..4).any(|index| pixel[index].abs_diff(other[index]) > tolerance);
        if differs {
            count += 1;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    (count > 0).then_some((count, (x0, y0, x1, y1)))
}
