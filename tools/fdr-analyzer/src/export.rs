//! Export functionality for CSV data and PNG graph screenshots.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::fdr_reader::{DataPoint, FileMetadata, ParameterSeries};

/// Export the selected parameter series to a CSV file.
pub fn export_csv<P: AsRef<Path>>(
    path: P,
    metadata: &FileMetadata,
    series_map: &BTreeMap<String, ParameterSeries>,
) -> Result<(), std::io::Error> {
    let file = File::create(path)?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(BufWriter::new(file));

    // Write metadata as comment rows (they'll appear before the header in many viewers).
    // CSV doesn't have native comments, so we encode metadata as a special first section.
    writer.write_record(["# FDR Analyzer Export"])?;
    writer.write_record([format!("# File: {:?}", metadata.path)])?;
    if let Some(ac) = metadata.aircraft_type {
        let ac_name = match ac {
            crate::fdr_reader::AircraftType::A320 => "A320",
            crate::fdr_reader::AircraftType::A380 => "A380",
        };
        writer.write_record([format!("# Aircraft: {}", ac_name)])?;
    }
    writer.write_record([format!(
        "# Records: {} ({:.2}s - {:.2}s)",
        metadata.total_records, metadata.time_start_s, metadata.time_end_s
    )])?;

    // Header row.
    writer.write_record(["time_s", "value"])?;

    // Flatten all series into a single column-pair format for simplicity.
    // We interleave with a separator so the user can distinguish which parameter each
    // block of rows belongs to.
    for (key, series) in series_map {
        if !series.is_empty() {
            // Write marker row for this parameter.
            writer.write_record([format!("## {}", key), "".to_string()])?;

            for pt in series {
                writer.write_record([format!("{:.4}", pt.time_s), format!("{:.6}", pt.value)])?;
            }
        }
    }

    writer.flush()?;
    Ok(())
}

/// Collect all data point values across multiple series and return as rows.
pub fn collect_csv_rows<'a>(
    series_map: &'a BTreeMap<String, ParameterSeries>,
) -> Vec<(&'a str, DataPoint)> {
    let mut rows = Vec::new();
    for (key, series) in series_map {
        for pt in series {
            rows.push((key.as_str(), pt.clone()));
        }
    }
    rows
}
