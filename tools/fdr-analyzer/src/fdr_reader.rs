//! FDR Reader — Two-phase loading with time index for efficient random access.
//!
//! Supports both native `.fdr` (gzip-compressed binary) and `.csv` files
//! produced by `fdr2csv`.

use flate2::bufread::GzDecoder;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::mem;
use std::path::PathBuf;

use crate::fdr_reader::bindings_320::A320FdrData;
use crate::fdr_reader::bindings_380::A380FdrData;

// ── Re-export bindgen-generated types for typed record reading ───────
#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code
)]
mod bindings_320 {
    use bytemuck::AnyBitPattern;
    include!(concat!(env!("OUT_DIR"), "/bindings_320.rs"));

    /// Composite FdrData for the A320 — mirrors fdr2csv's a320.rs layout.
    pub(super) struct A320FdrData {
        base: BaseData,
        specific: AircraftSpecificData,
        elac_1: ElacData,
        elac_2: ElacData,
        sec_1: SecData,
        sec_2: SecData,
        sec_3: SecData,
        fac_1: FacData,
        fac_2: FacData,
        fmgc_1: FmgcData,
        fadec_1: FadecData,
    }

    #[derive(Default)]
    struct ElacData {
        bus_outputs: base_elac_out_bus,
        discrete_outputs: base_elac_discrete_outputs,
        analog_outputs: base_elac_analog_outputs,
    }

    #[derive(Default)]
    struct SecData {
        bus_outputs: base_sec_out_bus,
        discrete_outputs: base_sec_discrete_outputs,
        analog_outputs: base_sec_analog_outputs,
    }

    #[derive(Default)]
    struct FacData {
        bus_outputs: base_fac_bus,
        discrete_outputs: base_fac_discrete_outputs,
        analog_outputs: base_fac_analog_outputs,
    }

    #[derive(Default)]
    struct FmgcData {
        logic: base_fmgc_logic_outputs,
        ap_fd_logic: base_fmgc_ap_fd_logic_outputs,
        ap_fd_outer_loops: ap_raw_output,
        athr: base_fmgc_athr_outputs,
        discrete_outputs: base_fmgc_discrete_outputs,
        bus_outputs: base_fmgc_bus_outputs,
        bus_inputs: base_fmgc_bus_inputs,
        discrete_inputs: base_fmgc_discrete_inputs,
        fms_inputs: base_fms_inputs,
    }

    #[derive(Default)]
    struct FadecData {
        bus_outputs: base_ecu_bus,
        outputs: athr_output,
    }
}

#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code
)]
mod bindings_380 {
    use bytemuck::AnyBitPattern;
    include!(concat!(env!("OUT_DIR"), "/bindings_380.rs"));

    /// Composite FdrData for the A380 — mirrors fdr2csv's a380.rs layout.
    pub(super) struct A380FdrData {
        base: BaseData,
        specific: AircraftSpecificData,
        prim_1: PrimData,
        prim_2: PrimData,
        prim_3: PrimData,
        sec_1: SecData,
        sec_2: SecData,
        sec_3: SecData,
        ap_sm: ap_sm_output,
        ap_law: ap_laws_output,
        athr: athr_out,
        fuel: FuelSystemData,
    }

    #[derive(Default)]
    struct PrimData {
        bus_outputs: base_prim_out_bus,
        discrete_outputs: base_prim_discrete_outputs,
        analog_outputs: base_prim_analog_outputs,
    }

    #[derive(Default)]
    struct SecData {
        bus_outputs: base_sec_out_bus,
        discrete_outputs: base_sec_discrete_outputs,
        analog_outputs: base_sec_analog_outputs,
    }
}

/// Aircraft type as inferred from the FDR interface version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AircraftType {
    A320,
    A380,
}

/// Metadata about an open FDR data source.
#[derive(Debug, Clone, Default)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub aircraft_type: Option<AircraftType>,
    pub interface_version: u64,
    pub total_records: usize,
    pub time_start_s: f64,
    pub time_end_s: f64,
}

/// A single named parameter field from an FDR record.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// Human-readable name (Title Case, prefix-stripped).
    pub display_name: String,
    /// Raw field path used internally (e.g. "base.simulation_time_s").
    pub key: String,
    /// Subsystem group for the parameter browser tree.
    pub group: String,
    /// Unit string derived from suffix conventions.
    pub unit: String,
}

/// A single data point at a given time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataPoint {
    pub time_s: f64,
    pub value: f64,
}

/// A series of data points for one parameter over a time range.
pub type ParameterSeries = Vec<DataPoint>;

/// Time-based index mapping record index → simulation time (seconds).
#[derive(Debug, Clone, Default)]
pub struct TimeIndex {
    times: Vec<f64>,
}

impl TimeIndex {
    /// Binary search for the first record at or after `time_s`.
    pub fn find_index(&self, time_s: f64) -> usize {
        self.times
            .partition_point(|&t| t < time_s)
            .min(self.times.len())
    }

    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    pub fn get_time(&self, index: usize) -> Option<f64> {
        self.times.get(index).copied()
    }

    pub fn time_range(&self) -> (f64, f64) {
        (
            *self.times.first().unwrap_or(&0.0),
            *self.times.last().unwrap_or(&0.0),
        )
    }
}

// ── Thread-safe data store ────────────────────────────────────────

pub struct FdrStore {
    inner: RwLock<Option<FdrFileState>>,
}

#[derive(Debug)]
pub struct FdrFileState {
    pub metadata: FileMetadata,
    pub time_index: TimeIndex,
    pub parameters: Vec<ParameterInfo>,
    /// Column-major data: one Vec<f64> per registered parameter.
    pub columns: BTreeMap<String, Vec<f64>>,
}

impl Default for FdrStore {
    fn default() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────

impl FdrStore {
    /// Open a file and build its time index.
    pub fn open_file(&self, path: &std::path::Path) -> io::Result<()> {
        log::info!("Opening file: {:?}", path);

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let state = if extension.to_lowercase() == "csv" {
            self.open_csv(path)?
        } else {
            self.open_fdr(path)?
        };

        log::info!(
            "Loaded {} records from {:.2}s to {:.2}s",
            state.metadata.total_records,
            state.metadata.time_start_s,
            state.metadata.time_end_s
        );

        let mut guard = self.inner.write();
        *guard = Some(state);
        Ok(())
    }

    /// Query specified parameters over a time range. Returns at most `max_points` per series.
    pub fn query_range(
        &self,
        param_keys: &[&str],
        time_start: f64,
        time_end: f64,
        max_points: usize,
    ) -> BTreeMap<String, ParameterSeries> {
        let guard = self.inner.read();
        if let Some(ref state) = *guard {
            query_from_columns(
                &state.time_index,
                &state.columns,
                param_keys,
                time_start,
                time_end,
                max_points,
            )
        } else {
            BTreeMap::new()
        }
    }

    /// Get the live TimeIndex for external timeline interaction.
    pub fn time_index(&self) -> Option<TimeIndex> {
        self.inner.read().as_ref().map(|s| s.time_index.clone())
    }

    pub fn metadata(&self) -> Option<FileMetadata> {
        self.inner.read().as_ref().map(|s| s.metadata.clone())
    }

    pub fn parameters(&self) -> Vec<ParameterInfo> {
        self.inner
            .read()
            .as_ref()
            .map(|s| s.parameters.clone())
            .unwrap_or_default()
    }

    pub fn is_open(&self) -> bool {
        self.inner.read().is_some()
    }

    /// Reset to no file loaded.
    pub fn close(&self) {
        let mut guard = self.inner.write();
        *guard = None;
    }
}

// ── FDR binary reader ─────────────────────────────────────────────

/// Interface version threshold — anything above this is A380, else A320.
const A380_MIN_VERSION: u64 = 3_800_000;

fn read_bytes<T: bytemuck::AnyBitPattern>(reader: &mut dyn Read) -> io::Result<T> {
    let size = std::mem::size_of::<T>();
    let mut buf = [0u8; size];
    reader.read_exact(&mut buf)?;
    Ok(bytemuck::pod_read_unaligned(buf.as_slice()))
}

impl FdrStore {
    fn open_fdr(&self, path: &std::path::Path) -> io::Result<FdrFileState> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut decoder = GzDecoder::new(reader);

        // Read interface version (first 8 bytes of the decompressed stream).
        let interface_version: u64 = read_bytes(&mut decoder)?;
        let aircraft_type = if interface_version > A380_MIN_VERSION {
            AircraftType::A380
        } else {
            AircraftType::A320
        };

        // Determine record size from the typed struct.
        let record_size = match aircraft_type {
            AircraftType::A320 => std::mem::size_of::<A320FdrData>(),
            AircraftType::A380 => std::mem::size_of::<A380FdrData>(),
        };

        // Read all decompressed bytes.
        let mut all_bytes = Vec::new();
        decoder.read_to_end(&mut all_bytes)?;

        if all_bytes.len() < record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "File too small to contain even one FDR record",
            ));
        }

        let total_records = all_bytes.len() / record_size;
        all_bytes.truncate(total_records * record_size);

        // Extract timestamps (first 8 bytes of each record are simulation_time_s as f64).
        let mut times: Vec<f64> = Vec::with_capacity(total_records);
        for i in 0..total_records {
            let offset = i * record_size;
            let time_bytes: [u8; 8] = all_bytes[offset..offset + 8].try_into().unwrap_or_default();
            times.push(f64::from_ne_bytes(time_bytes));
        }

        // Build column-major data for registered parameters.
        let (parameters, columns) = match aircraft_type {
            AircraftType::A320 => self.read_a320_columns(&all_bytes, record_size, total_records),
            AircraftType::A380 => self.read_a380_columns(&all_bytes, record_size, total_records),
        };

        let time_start = *times.first().unwrap_or(&0.0);
        let time_end = *times.last().unwrap_or(&0.0);

        Ok(FdrFileState {
            metadata: FileMetadata {
                path: path.to_path_buf(),
                aircraft_type: Some(aircraft_type),
                interface_version,
                total_records,
                time_start_s: time_start,
                time_end_s: time_end,
            },
            time_index: TimeIndex { times },
            parameters,
            columns,
        })
    }

    /// Read all registered A320 parameters into column-major storage.
    fn read_a320_columns(
        &self,
        data: &[u8],
        record_size: usize,
        total_records: usize,
    ) -> (Vec<ParameterInfo>, BTreeMap<String, Vec<f64>>) {
        // We read full FdrData records at known byte offsets and extract fields.
        // This is safe because the buffer is guaranteed to contain complete records.

        let mut cols = BTreeMap::new();

        // Helper: extract f64 at a given field offset from each record.
        let make_column = |offset: usize| -> Vec<f64> {
            (0..total_records)
                .map(|i| {
                    let base = i * record_size + offset;
                    if base + 8 <= data.len() {
                        let bytes: [u8; 8] = data[base..base + 8].try_into().unwrap_or_default();
                        f64::from_ne_bytes(bytes)
                    } else {
                        0.0
                    }
                })
                .collect()
        };

        // BaseData fields (offsets from C++ struct layout: each double = 8 bytes).
        cols.insert("base.simulation_time_s".into(), make_column(0));
        cols.insert("base.simulation_delta_time_s".into(), make_column(8));
        cols.insert("base.simulation_rate".into(), make_column(16));
        cols.insert(
            "base.aircraft_position_latitude_deg".into(),
            make_column(56),
        );
        cols.insert(
            "base.aircraft_position_longitude_deg".into(),
            make_column(64),
        );
        cols.insert("base.aircraft_Theta_deg".into(), make_column(80));
        cols.insert("base.aircraft_Phi_deg".into(), make_column(88));
        cols.insert("base.aircraft_Psi_magnetic_deg".into(), make_column(96));
        cols.insert("base.aircraft_qk_deg_s".into(), make_column(120));
        cols.insert("base.aircraft_pk_deg_s".into(), make_column(128));
        cols.insert("base.aircraft_rk_deg_s".into(), make_column(136));
        cols.insert("base.aircraft_V_indicated_kn".into(), make_column(144));
        cols.insert("base.aircraft_V_true_kn".into(), make_column(152));
        cols.insert("base.aircraft_V_ground_kn".into(), make_column(160));
        cols.insert("base.aircraft_Ma_mach".into(), make_column(168));
        cols.insert("base.aircraft_alpha_deg".into(), make_column(176));
        cols.insert("base.aircraft_beta_deg".into(), make_column(184));
        cols.insert("base.aircraft_H_pressure_ft".into(), make_column(200));
        cols.insert("base.aircraft_nz_g".into(), make_column(232));
        cols.insert(
            "base.atmosphere_ambient_pressure_mbar".into(),
            make_column(296),
        );

        let params = build_a320_params();
        (params, cols)
    }

    /// Read all registered A380 parameters into column-major storage.
    fn read_a380_columns(
        &self,
        data: &[u8],
        record_size: usize,
        total_records: usize,
    ) -> (Vec<ParameterInfo>, BTreeMap<String, Vec<f64>>) {
        let mut cols = BTreeMap::new();

        let make_column = |offset: usize| -> Vec<f64> {
            (0..total_records)
                .map(|i| {
                    let base = i * record_size + offset;
                    if base + 8 <= data.len() {
                        let bytes: [u8; 8] = data[base..base + 8].try_into().unwrap_or_default();
                        f64::from_ne_bytes(bytes)
                    } else {
                        0.0
                    }
                })
                .collect()
        };

        // BaseData has the same layout for A380.
        cols.insert("base.simulation_time_s".into(), make_column(0));
        cols.insert("base.simulation_delta_time_s".into(), make_column(8));
        cols.insert("base.simulation_rate".into(), make_column(16));
        cols.insert(
            "base.aircraft_position_latitude_deg".into(),
            make_column(56),
        );
        cols.insert(
            "base.aircraft_position_longitude_deg".into(),
            make_column(64),
        );
        cols.insert("base.aircraft_Theta_deg".into(), make_column(80));
        cols.insert("base.aircraft_Phi_deg".into(), make_column(88));
        cols.insert("base.aircraft_Psi_magnetic_deg".into(), make_column(96));
        cols.insert("base.aircraft_qk_deg_s".into(), make_column(120));
        cols.insert("base.aircraft_pk_deg_s".into(), make_column(128));
        cols.insert("base.aircraft_rk_deg_s".into(), make_column(136));
        cols.insert("base.aircraft_V_indicated_kn".into(), make_column(144));
        cols.insert("base.aircraft_V_true_kn".into(), make_column(152));
        cols.insert("base.aircraft_V_ground_kn".into(), make_column(160));
        cols.insert("base.aircraft_Ma_mach".into(), make_column(168));
        cols.insert("base.aircraft_alpha_deg".into(), make_column(176));
        cols.insert("base.aircraft_beta_deg".into(), make_column(184));
        cols.insert("base.aircraft_H_pressure_ft".into(), make_column(200));
        cols.insert("base.aircraft_nz_g".into(), make_column(232));

        let params = build_a380_params();
        (params, cols)
    }
}

// ── CSV reader ────────────────────────────────────────────────────

impl FdrStore {
    fn open_csv(&self, path: &std::path::Path) -> io::Result<FdrFileState> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_path(path)?;

        let headers = rdr.headers()?.clone();

        let time_col_index = headers
            .iter()
            .position(|h| h.contains("simulation_time_s"))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "No time column found in CSV")
            })?;

        let num_columns = headers.len();
        let mut columns: Vec<Vec<f64>> = vec![Vec::new(); num_columns];

        for result in rdr.records() {
            let record = result?;
            for col_idx in 0..num_columns {
                if let Some(field) = record.get(col_idx) {
                    if let Ok(val) = field.parse::<f64>() {
                        columns[col_idx].push(val);
                    } else {
                        columns[col_idx].push(0.0);
                    }
                } else {
                    columns[col_idx].push(0.0);
                }
            }
        }

        let row_count = if columns.is_empty() {
            0
        } else {
            columns[0].len()
        };

        let times: Vec<f64> = std::mem::take(&mut columns[time_col_index]);
        columns[time_col_index] = Vec::new();

        let time_start = *times.first().unwrap_or(&0.0);
        let time_end = *times.last().unwrap_or(&0.0);

        let params: Vec<ParameterInfo> = headers
            .iter()
            .map(|h| {
                let (display_name, unit) = parse_header_name(h);
                ParameterInfo {
                    key: h.to_string(),
                    group: infer_group(h),
                    display_name,
                    unit,
                }
            })
            .collect();

        // Convert to BTreeMap keyed by header name.
        let mut col_map = BTreeMap::new();
        for (i, h) in headers.iter().enumerate() {
            col_map.insert(h.to_string(), columns[i].clone());
        }

        Ok(FdrFileState {
            metadata: FileMetadata {
                path: path.to_path_buf(),
                aircraft_type: None,
                interface_version: 0,
                total_records: row_count,
                time_start_s: time_start,
                time_end_s: time_end,
            },
            time_index: TimeIndex { times },
            parameters: params,
            columns: col_map,
        })
    }
}

// ── Query from column-major storage ───────────────────────────────

fn query_from_columns(
    time_index: &TimeIndex,
    columns: &BTreeMap<String, Vec<f64>>,
    param_keys: &[&str],
    time_start: f64,
    time_end: f64,
    max_points: usize,
) -> BTreeMap<String, ParameterSeries> {
    let idx_start = time_index.find_index(time_start);
    let idx_end = time_index
        .find_index(time_end)
        .saturating_add(1)
        .min(time_index.len());

    if idx_start >= idx_end {
        return BTreeMap::new();
    }

    let total_range = idx_end - idx_start;
    let step = if total_range > max_points {
        total_range / max_points
    } else {
        1
    };

    let mut result = BTreeMap::new();

    for key in param_keys {
        if let Some(col) = columns.get(*key) {
            let mut series = Vec::with_capacity((idx_end - idx_start) / step + 1);
            for i in (idx_start..idx_end).step_by(step) {
                if i >= col.len() {
                    break;
                }
                if let Some(time_s) = time_index.get_time(i) {
                    series.push(DataPoint {
                        time_s,
                        value: col[i],
                    });
                }
            }
            result.insert((*key).to_string(), series);
        }
    }

    result
}

// ── Parameter definition helpers ──────────────────────────────────

fn make_param(key: &str, group: &str, name: &str, unit: &str) -> ParameterInfo {
    ParameterInfo {
        key: key.to_string(),
        group: group.to_string(),
        display_name: name.to_string(),
        unit: unit.to_string(),
    }
}

fn build_a320_params() -> Vec<ParameterInfo> {
    vec![
        make_param("base.simulation_time_s", "Simulation", "Time", "s"),
        make_param(
            "base.simulation_delta_time_s",
            "Simulation",
            "Delta Time",
            "s",
        ),
        make_param("base.simulation_rate", "Simulation", "Sim Rate", ""),
        make_param(
            "base.aircraft_position_latitude_deg",
            "Position",
            "Latitude",
            "°",
        ),
        make_param(
            "base.aircraft_position_longitude_deg",
            "Position",
            "Longitude",
            "°",
        ),
        make_param("base.aircraft_Theta_deg", "Attitude", "Pitch", "°"),
        make_param("base.aircraft_Phi_deg", "Attitude", "Roll", "°"),
        make_param(
            "base.aircraft_Psi_magnetic_deg",
            "Attitude",
            "Heading (Mag)",
            "°",
        ),
        make_param("base.aircraft_qk_deg_s", "Attitude", "Pitch Rate", "°/s"),
        make_param("base.aircraft_pk_deg_s", "Attitude", "Roll Rate", "°/s"),
        make_param("base.aircraft_rk_deg_s", "Attitude", "Yaw Rate", "°/s"),
        make_param(
            "base.aircraft_V_indicated_kn",
            "Flight Dynamics",
            "Indicated Airspeed",
            "kt",
        ),
        make_param(
            "base.aircraft_V_true_kn",
            "Flight Dynamics",
            "True Airspeed",
            "kt",
        ),
        make_param(
            "base.aircraft_V_ground_kn",
            "Flight Dynamics",
            "Ground Speed",
            "kt",
        ),
        make_param(
            "base.aircraft_Ma_mach",
            "Flight Dynamics",
            "Mach Number",
            "Mach",
        ),
        make_param(
            "base.aircraft_alpha_deg",
            "Flight Dynamics",
            "Angle of Attack",
            "°",
        ),
        make_param(
            "base.aircraft_beta_deg",
            "Flight Dynamics",
            "Sideslip Angle",
            "°",
        ),
        make_param(
            "base.aircraft_H_pressure_ft",
            "Flight Dynamics",
            "Pressure Altitude",
            "ft",
        ),
        make_param(
            "base.aircraft_nz_g",
            "Flight Dynamics",
            "Vertical Acceleration",
            "g",
        ),
        make_param(
            "base.atmosphere_ambient_pressure_mbar",
            "Atmosphere",
            "Ambient Pressure",
            "mbar",
        ),
    ]
}

fn build_a380_params() -> Vec<ParameterInfo> {
    vec![
        make_param("base.simulation_time_s", "Simulation", "Time", "s"),
        make_param(
            "base.simulation_delta_time_s",
            "Simulation",
            "Delta Time",
            "s",
        ),
        make_param("base.simulation_rate", "Simulation", "Sim Rate", ""),
        make_param(
            "base.aircraft_position_latitude_deg",
            "Position",
            "Latitude",
            "°",
        ),
        make_param(
            "base.aircraft_position_longitude_deg",
            "Position",
            "Longitude",
            "°",
        ),
        make_param("base.aircraft_Theta_deg", "Attitude", "Pitch", "°"),
        make_param("base.aircraft_Phi_deg", "Attitude", "Roll", "°"),
        make_param(
            "base.aircraft_Psi_magnetic_deg",
            "Attitude",
            "Heading (Mag)",
            "°",
        ),
        make_param("base.aircraft_qk_deg_s", "Attitude", "Pitch Rate", "°/s"),
        make_param("base.aircraft_pk_deg_s", "Attitude", "Roll Rate", "°/s"),
        make_param("base.aircraft_rk_deg_s", "Attitude", "Yaw Rate", "°/s"),
        make_param(
            "base.aircraft_V_indicated_kn",
            "Flight Dynamics",
            "Indicated Airspeed",
            "kt",
        ),
        make_param(
            "base.aircraft_V_true_kn",
            "Flight Dynamics",
            "True Airspeed",
            "kt",
        ),
        make_param(
            "base.aircraft_V_ground_kn",
            "Flight Dynamics",
            "Ground Speed",
            "kt",
        ),
        make_param(
            "base.aircraft_Ma_mach",
            "Flight Dynamics",
            "Mach Number",
            "Mach",
        ),
        make_param(
            "base.aircraft_alpha_deg",
            "Flight Dynamics",
            "Angle of Attack",
            "°",
        ),
        make_param(
            "base.aircraft_beta_deg",
            "Flight Dynamics",
            "Sideslip Angle",
            "°",
        ),
        make_param(
            "base.aircraft_H_pressure_ft",
            "Flight Dynamics",
            "Pressure Altitude",
            "ft",
        ),
        make_param(
            "base.aircraft_nz_g",
            "Flight Dynamics",
            "Vertical Acceleration",
            "g",
        ),
    ]
}

/// Parse a CSV header name like "base.aircraft_V_indicated_kn" into (Display Name, Unit).
fn parse_header_name(header: &str) -> (String, String) {
    let parts: Vec<&str> = header.split('.').collect();
    let leaf = parts.last().unwrap_or(&header);
    let (clean_name, unit_suffix) = split_unit_suffix(leaf);
    let display = to_title_case(clean_name);
    (display, format_unit(unit_suffix))
}

fn infer_group(header: &str) -> String {
    let parts: Vec<&str> = header.split('.').collect();
    if parts.len() >= 2 {
        match parts[0] {
            "base" => match parts.get(1) {
                Some(s) if s.starts_with("aircraft_position") => "Position".to_string(),
                Some(s)
                    if s.starts_with("aircraft_Theta")
                        || s.starts_with("aircraft_Phi")
                        || s.starts_with("aircraft_Psi") =>
                {
                    "Attitude".to_string()
                }
                _ => "Base".to_string(),
            },
            "specific" => "Aircraft Systems".to_string(),
            other => other.to_uppercase(),
        }
    } else {
        "Other".to_string()
    }
}

const UNIT_MAP: &[(&str, &str)] = &[
    ("deg_s", "°/s"),
    ("deg", "°"),
    ("kn", "kt"),
    ("ft", "ft"),
    ("m_s2", "m/s²"),
    ("psi", "psi"),
    ("mach", "Mach"),
    ("percent", "%"),
    ("g", "g"),
    ("mbar", "mbar"),
    ("s", "s"),
];

fn split_unit_suffix(name: &str) -> (&str, &str) {
    // Try longest suffix first for more accurate matching.
    let sorted: Vec<_> = UNIT_MAP.iter().collect();
    for &(suffix, _unit) in sorted.iter().rev() {
        if name.ends_with(&format!("_{}", suffix)) {
            let cut = name.len() - suffix.len() - 1; // Also strip the underscore
            return (&name[..cut], suffix);
        }
    }
    (name, "")
}

fn format_unit(suffix: &str) -> String {
    UNIT_MAP
        .iter()
        .find(|(s, _)| *s == suffix)
        .map(|(_, u)| (*u).to_string())
        .unwrap_or_default()
}

fn to_title_case(name: &str) -> String {
    name.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
