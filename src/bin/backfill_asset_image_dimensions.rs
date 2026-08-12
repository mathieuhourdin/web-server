use std::env;

use web_server::{
    db,
    entities_v2::platform_infra::asset::{
        download_asset_bytes_from_gcs, extract_image_dimensions, Asset,
    },
};

fn parse_batch_size() -> Result<i64, String> {
    let mut args = env::args().skip(1);
    let Some(flag) = args.next() else {
        return Ok(100);
    };
    if flag != "--batch-size" {
        return Err("Usage: backfill_asset_image_dimensions [--batch-size <1..=1000>]".to_string());
    }
    let value = args
        .next()
        .ok_or_else(|| "--batch-size requires a value".to_string())?;
    if args.next().is_some() {
        return Err("Usage: backfill_asset_image_dimensions [--batch-size <1..=1000>]".to_string());
    }
    let batch_size = value
        .parse::<i64>()
        .map_err(|_| "--batch-size must be an integer".to_string())?;
    if !(1..=1000).contains(&batch_size) {
        return Err("--batch-size must be between 1 and 1000".to_string());
    }
    Ok(batch_size)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let batch_size = parse_batch_size()?;
    let pool = db::create_pool();
    let assets = Asset::list_image_dimensions_backfill_batch(batch_size, &pool)
        .map_err(|error| error.message)?;
    if assets.is_empty() {
        println!("No image assets require dimension backfill.");
        return Ok(());
    }

    let mut updated = 0;
    let mut failed = 0;
    for asset in assets {
        match download_asset_bytes_from_gcs(&asset.bucket, &asset.object_key).await {
            Ok(bytes) => match extract_image_dimensions(&asset.mime_type, &bytes) {
                Ok(Some((width, height))) => {
                    Asset::update_image_dimensions(asset.id, width, height, &pool)
                        .map_err(|error| error.message)?;
                    updated += 1;
                    println!(
                        "updated asset_id={} width={} height={}",
                        asset.id, width, height
                    );
                }
                Ok(None) => {
                    failed += 1;
                    eprintln!("skipped asset_id={} because it is not an image", asset.id);
                }
                Err(error) => {
                    failed += 1;
                    eprintln!("failed asset_id={} message={}", asset.id, error.message);
                }
            },
            Err(error) => {
                failed += 1;
                eprintln!("failed asset_id={} message={}", asset.id, error.message);
            }
        }
    }
    println!("backfill complete updated={} failed={}", updated, failed);
    Ok(())
}
