use std::fmt;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use crate::{Close, High, Low, Next, Reset, Volume};

#[derive(Debug)]
pub enum VolumeWeightedAveragePriceBands {
    Up,
    Down,
}

/// Volume Weighted Average Price (VWAP)
///
/// VWAP equals the dollar value of all trading periods divided
/// by the total trading volume for the current day.
/// The calculation starts when trading opens and ends when it closes.
/// Because it is good for the current trading day only,
/// intraday periods and data are used in the calculation.
///
/// # Standard Deviation and Bands
///
/// The standard deviation calculation requires at least 2 data points to produce
/// meaningful values. With a single data point:
/// - The variance calculation will result in zero
/// - The standard deviation will be zero
/// - Band calculations (VWAP ± offset * std_dev) will equal VWAP
/// - Upper and lower bands will be identical to VWAP until second data point is added
#[doc(alias = "VWAP")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct VolumeWeightedAveragePrice {
    window: usize,
    price_volume_history: Vec<f64>,
    volume_history: Vec<f64>,
    vwap: f64,
    std_dev: f64,
}

impl VolumeWeightedAveragePrice {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            price_volume_history: Vec::with_capacity(window),
            volume_history: Vec::with_capacity(window),
            vwap: 0.0,
            std_dev: 0.0,
        }
    }

    pub fn vwap(&self) -> f64 {
        self.vwap
    }

    pub fn std_dev(&self, offset: f64, band_direction: VolumeWeightedAveragePriceBands) -> f64 {
        match band_direction {
            VolumeWeightedAveragePriceBands::Up => self.vwap + offset * self.std_dev,
            VolumeWeightedAveragePriceBands::Down => self.vwap - offset * self.std_dev,
        }
    }

    fn update_vwap(&mut self) {
        let total_pv: f64 = self.price_volume_history.iter().sum();
        let total_volume: f64 = self.volume_history.iter().sum();

        if total_volume > 0.0 {
            self.vwap = total_pv / total_volume;
        }
    }
}

impl<T: High + Low + Close + Volume> Next<&T> for VolumeWeightedAveragePrice {
    type Output = f64;

    fn next(&mut self, input: &T) -> Self::Output {
        let typical_price = (input.high() + input.low() + input.close()) / 3.0;
        let price_volume = typical_price * input.volume();

        self.price_volume_history.push(price_volume);
        self.volume_history.push(input.volume());

        if self.price_volume_history.len() > self.window {
            self.price_volume_history.remove(0);
            self.volume_history.remove(0);
        }

        self.update_vwap();

        // Calculate standard deviation
        if self.volume_history.len() >= 2 {
            let mean = self.vwap;
            let variance: f64 = self.price_volume_history.iter()
                .zip(&self.volume_history)
                .map(|(&pv, &v)| {
                    let x = pv / v;
                    (x - mean).powi(2)
                })
                .sum::<f64>() / (self.volume_history.len() as f64);
            self.std_dev = variance.sqrt();
        }

        self.vwap
    }
}

impl Reset for VolumeWeightedAveragePrice {
    fn reset(&mut self) {
        self.price_volume_history.clear();
        self.volume_history.clear();
        self.vwap = 0.0;
        self.std_dev = 0.0;
    }
}

impl Default for VolumeWeightedAveragePrice {
    fn default() -> Self {
        Self::new(14)
    }
}


impl fmt::Display for VolumeWeightedAveragePrice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VWAP({})", self.window)
    }
}


#[cfg(test)]
mod tests {
    use crate::DataItem;
    use super::*;
    use crate::test_helper::*;

    test_hlcv_indicator!(VolumeWeightedAveragePrice);
    #[test]
    fn test_new() {
        let vwap = VolumeWeightedAveragePrice::new(14);
        assert_eq!(vwap.window, 14);
        assert_eq!(vwap.vwap, 0.0);
        assert_eq!(vwap.std_dev, 0.0);
        assert!(vwap.price_volume_history.is_empty());
        assert!(vwap.volume_history.is_empty());
    }

    #[test]
    fn test_next() {
        let mut vwap = VolumeWeightedAveragePrice::new(3);

        let bar1 = DataItem::builder()
            .open(8.0)
            .high(10.0)
            .low(8.0)
            .close(9.0)
            .volume(100.0)
            .build()
            .unwrap();

        let bar2 = DataItem::builder()
            .open(10.0)
            .high(12.0)
            .low(10.0)
            .close(11.0)
            .volume(150.0)
            .build()
            .unwrap();

        let bar3 = DataItem::builder()
            .open(11.0)
            .high(13.0)
            .low(11.0)
            .close(12.0)
            .volume(200.0)
            .build()
            .unwrap();

        let result1 = vwap.next(&bar1);
        let expected1 = 9.0; // (10 + 8 + 9) / 3 * 100 / 100
        assert!((result1 - expected1).abs() < 0.0001);

        let result2 = vwap.next(&bar2);
        // Calculate expected VWAP for two periods
        let expected2 = ((9.0 * 100.0) + (11.0 * 150.0)) / (100.0 + 150.0);
        assert!((result2 - expected2).abs() < 0.0001);

        let result3 = vwap.next(&bar3);
        // Calculate expected VWAP for three periods
        let expected3 = ((9.0 * 100.0) + (11.0 * 150.0) + (12.0 * 200.0)) / (100.0 + 150.0 + 200.0);
        assert!((result3 - expected3).abs() < 0.0001);
    }

    #[test]
    fn test_bands() {
        let mut vwap = VolumeWeightedAveragePrice::new(3);

        // First data point
        let bar1 = DataItem::builder()
            .open(8.0)
            .high(10.0)
            .low(8.0)
            .close(9.0)
            .volume(100.0)
            .build()
            .unwrap();

        // Second data point with different values
        let bar2 = DataItem::builder()
            .open(9.0)
            .high(12.0)
            .low(9.0)
            .close(11.0)
            .volume(150.0)
            .build()
            .unwrap();

        vwap.next(&bar1);
        vwap.next(&bar2);

        let upper_band = vwap.std_dev(2.0, VolumeWeightedAveragePriceBands::Up);
        let lower_band = vwap.std_dev(2.0, VolumeWeightedAveragePriceBands::Down);

        assert!(upper_band > vwap.vwap());
        assert!(lower_band < vwap.vwap());
    }

    #[test]
    fn test_default() {
        let vwap = VolumeWeightedAveragePrice::default();
        assert_eq!(vwap.window, 14);
    }

    #[test]
    fn test_display() {
        let vwap = VolumeWeightedAveragePrice::new(7);
        assert_eq!(format!("{}", vwap), "VWAP(7)");
    }

    #[test]
    fn test_window_size() {
        let mut vwap = VolumeWeightedAveragePrice::new(2);

        let bar1 = DataItem::builder()
            .open(8.0)
            .high(10.0)
            .low(8.0)
            .close(9.0)
            .volume(100.0)
            .build()
            .unwrap();

        let bar2 = DataItem::builder()
            .open(10.0)
            .high(12.0)
            .low(10.0)
            .close(11.0)
            .volume(150.0)
            .build()
            .unwrap();

        let bar3 = DataItem::builder()
            .open(11.0)
            .high(13.0)
            .low(11.0)
            .close(12.0)
            .volume(200.0)
            .build()
            .unwrap();

        vwap.next(&bar1);
        vwap.next(&bar2);
        vwap.next(&bar3);

        assert_eq!(vwap.price_volume_history.len(), 2);
        assert_eq!(vwap.volume_history.len(), 2);
    }
}