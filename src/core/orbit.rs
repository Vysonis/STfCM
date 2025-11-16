use tracing::debug;

/// Propagate elements by a given number of minutes using SGP4.
pub fn propagate_minutes(elements: &sgp4::Elements, minutes: f64) -> Result<sgp4::Prediction, sgp4::Error> {
    let constants = sgp4::Constants::from_elements(elements)?;
    debug!(minutes, "Propagating elements");
    constants.propagate(minutes)
}