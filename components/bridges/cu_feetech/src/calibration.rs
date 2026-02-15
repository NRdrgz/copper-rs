//! Calibration data and unit conversions for Feetech servos.
//!
//! Each servo has a recorded min and max raw position. The center is
//! `(min + max) / 2` and is used as the zero reference when converting
//! to degrees or radians.
//!
//! Run the `feetech-calibrate` binary to generate a `calibration.json`.

use cu29::units::si::angle::{degree, radian};
use cu29::units::si::f32::Angle;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Output unit for published positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Units {
    /// Raw 12-bit register values (0–4095).  No calibration needed.
    #[default]
    Raw,
    /// Degrees relative to the calibration center (0 = center).
    Deg,
    /// Radians relative to the calibration center (0 = center).
    Rad,
}

impl Units {
    /// Parse from a config string.  Returns `None` for unrecognised values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "raw" => Some(Self::Raw),
            "deg" => Some(Self::Deg),
            "rad" => Some(Self::Rad),
            _ => None,
        }
    }
}

// =========================================================================
// Conversion helpers
// =========================================================================

/// Full-circle scale: 4096 ticks = 360° for STS3215.
const TICKS_PER_REV: f32 = 4096.0;

impl Units {
    /// Convert a raw 12-bit tick to the output unit.
    ///
    /// `center` is the calibration midpoint (in raw ticks) for this servo.
    /// Ignored when `self == Raw`.  For `Deg` and `Rad` uses [`cu29::units`]
    /// for type-safe angle conversion.
    #[inline]
    pub fn from_raw(self, raw: u16, center: f32) -> f32 {
        match self {
            Self::Raw => raw as f32,
            Self::Deg => {
                let deg = (raw as f32 - center) * 360.0 / TICKS_PER_REV;
                Angle::new::<degree>(deg).get::<degree>()
            }
            Self::Rad => {
                let rad = (raw as f32 - center) * core::f32::consts::TAU / TICKS_PER_REV;
                Angle::new::<radian>(rad).get::<radian>()
            }
        }
    }

    /// Convert an output-unit value back to a raw 12-bit tick.
    ///
    /// Result is clamped to `0..=4095`.  For `Deg` and `Rad`, the input
    /// value is interpreted via [`cu29::units::Angle`].
    #[inline]
    pub fn to_raw(self, value: f32, center: f32) -> u16 {
        let raw = match self {
            Self::Raw => value,
            Self::Deg => {
                let deg = Angle::new::<degree>(value).get::<degree>();
                deg * TICKS_PER_REV / 360.0 + center
            }
            Self::Rad => {
                let rad = Angle::new::<radian>(value).get::<radian>();
                rad * TICKS_PER_REV / core::f32::consts::TAU + center
            }
        };
        raw.round().clamp(0.0, 4095.0) as u16
    }
}

// =========================================================================
// Per-servo calibration
// =========================================================================

/// Calibration for a single servo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServoCalibration {
    pub id: u8,
    pub min: u16,
    pub max: u16,
}

impl ServoCalibration {
    /// Midpoint between min and max — the "zero" position.
    pub fn center(&self) -> f32 {
        (self.min as f32 + self.max as f32) / 2.0
    }

    /// Total usable range in raw ticks.
    pub fn range(&self) -> u16 {
        self.max.saturating_sub(self.min)
    }
}

/// Calibration data for all servos on a bus.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationData {
    pub servos: Vec<ServoCalibration>,
}

impl CalibrationData {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| std::io::Error::other(format!("bad calibration JSON: {e}")))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }

    /// Look up the center (midpoint) for a servo by bus ID.
    ///
    /// Returns `None` if no calibration entry exists for that ID.
    pub fn center_for(&self, id: u8) -> Option<f32> {
        self.servos.iter().find(|s| s.id == id).map(|s| s.center())
    }
}
