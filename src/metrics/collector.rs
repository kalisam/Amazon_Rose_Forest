//! Metrics collection system for monitoring system performance
//! Provides insights into query latency, throughput, and resource utilization

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use std::sync::atomic::{AtomicUsize, Ordering};

static LOCK_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

/// Types of metrics that can be collected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    /// Count of events (incremental)
    Counter,
    /// Value at a point in time
    Gauge,
    /// Distribution of values over time
    Histogram,
}

/// A single metric value with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    /// The value of the metric
    pub value: f64,
    /// When the metric was recorded
    pub timestamp: u64,
}

/// Configuration for histogram metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramConfig {
    /// Minimum value to track
    pub min: f64,
    /// Maximum value to track
    pub max: f64,
    /// Number of buckets in the histogram
    pub buckets: usize,
}

impl Default for HistogramConfig {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 1000.0,
            buckets: 10,
        }
    }
}

/// A histogram for tracking distribution of values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    /// Configuration for the histogram
    pub config: HistogramConfig,
    /// Buckets for the histogram
    pub buckets: Vec<u64>,
    /// Count of values in the histogram
    pub count: u64,
    /// Sum of all values
    pub sum: f64,
    /// Minimum observed value
    pub min: f64,
    /// Maximum observed value
    pub max: f64,
}

impl Histogram {
    /// Create a new histogram with the specified configuration
    pub fn new(config: HistogramConfig) -> Self {
        Self {
            buckets: vec![0; config.buckets],
            count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            config,
        }
    }

    /// Record a value in the histogram
    pub fn record(&mut self, value: f64) {
        // Update basic statistics
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        // Determine bucket index
        if value < self.config.min {
            self.buckets[0] += 1;
        } else if value >= self.config.max {
            self.buckets[self.config.buckets - 1] += 1;
        } else {
            let bucket_width = (self.config.max - self.config.min) / self.config.buckets as f64;
            let bucket_index = ((value - self.config.min) / bucket_width) as usize;
            self.buckets[bucket_index.min(self.config.buckets - 1)] += 1;
        }
    }

    /// Get the average value
    pub fn average(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Get the percentile value (0.0 to 1.0)
    pub fn percentile(&self, p: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        let target = (self.count as f64 * p) as u64;
        let mut count = 0;

        for (i, &bucket_count) in self.buckets.iter().enumerate() {
            count += bucket_count;
            if count >= target {
                // Estimate the value within this bucket
                let bucket_width = (self.config.max - self.config.min) / self.config.buckets as f64;
                let bucket_start = self.config.min + (i as f64 * bucket_width);
                return bucket_start + (bucket_width / 2.0); // Return midpoint of bucket as estimate
            }
        }

        self.max // Fallback
    }
}

/// Metrics collector for system monitoring
pub struct MetricsCollector {
    /// Counter metrics
    counters: Arc<Mutex<HashMap<String, Vec<MetricValue>>>>,
    /// Gauge metrics
    gauges: Arc<Mutex<HashMap<String, Vec<MetricValue>>>>,
    /// Histogram metrics
    histograms: Arc<Mutex<HashMap<String, Histogram>>>,
    /// Start time of the collector
    start_time: Instant,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(Mutex::new(HashMap::new())),
            gauges: Arc::new(Mutex::new(HashMap::new())),
            histograms: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// Increment a counter metric
    pub fn increment_counter(&self, key: &str, value: f64) -> Result<(), MetricsError> {
        let mut counters = try_lock_with_backoff(&*self.counters).map_err(|_| MetricsError::LockPoisoned)?;
        let counter = counters.entry(key.to_string()).or_insert_with(Vec::new);

        let timestamp = self.elapsed_millis();
        counter.push(MetricValue { value, timestamp });

        Ok(())
    }

    /// Set a gauge metric
    pub fn set_gauge(&self, key: &str, value: f64) -> Result<(), MetricsError> {
        let mut gauges = try_lock_with_backoff(&*self.gauges).map_err(|_| MetricsError::LockPoisoned)?;
        let gauge = gauges.entry(key.to_string()).or_insert_with(Vec::new);

        let timestamp = self.elapsed_millis();
        gauge.push(MetricValue { value, timestamp });

        Ok(())
    }

    /// Record a value in a histogram
    pub fn record_histogram(&self, key: &str, value: f64, config: Option<HistogramConfig>) -> Result<(), MetricsError> {
        let mut histograms = try_lock_with_backoff(&*self.histograms).map_err(|_| MetricsError::LockPoisoned)?;
        let histogram = histograms.entry(key.to_string()).or_insert_with(|| {
            Histogram::new(config.unwrap_or_default())
        });

        histogram.record(value);

        Ok(())
    }

    /// Record the duration of an operation
    pub fn record_duration<F, T>(&self, key: &str, f: F) -> Result<T, MetricsError>
    where
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();

        self.record_histogram(key, duration.as_millis() as f64, None)?;

        Ok(result)
    }

    /// Get the average value of a counter
    pub fn counter_average(&self, key: &str) -> Option<f64> {
        let counters = try_lock_with_backoff(&*self.counters).map_err(|_| MetricsError::LockPoisoned).ok()?;
        counters.get(key).map(|values| {
            if values.is_empty() {
                0.0
            } else {
                values.iter().map(|v| v.value).sum::<f64>() / values.len() as f64
            }
        })
    }

    /// Get the latest value of a gauge
    pub fn gauge_latest(&self, key: &str) -> Option<f64> {
        let gauges = try_lock_with_backoff(&*self.gauges).map_err(|_| MetricsError::LockPoisoned).ok()?;
        gauges.get(key).and_then(|values| {
            values.last().map(|v| v.value)
        })
    }

    /// Get a histogram
    pub fn get_histogram(&self, key: &str) -> Option<Histogram> {
        let histograms = try_lock_with_backoff(&*self.histograms).map_err(|_| MetricsError::LockPoisoned).ok()?;
        histograms.get(key).cloned()
    }

    /// Get all metrics as a serializable structure
    pub fn get_all_metrics(&self) -> Result<HashMap<String, serde_json::Value>, MetricsError> {
        let mut result = HashMap::new();

        // Add counters
        {
            let counters = try_lock_with_backoff(&*self.counters).map_err(|_| MetricsError::LockPoisoned)?;
            for (key, values) in counters.iter() {
                if let Ok(json) = serde_json::to_value(values) {
                    result.insert(format!("counter.{}", key), json);
                }
            }
        }

        // Add gauges
        {
            let gauges = try_lock_with_backoff(&*self.gauges).map_err(|_| MetricsError::LockPoisoned)?;
            for (key, values) in gauges.iter() {
                if let Ok(json) = serde_json::to_value(values) {
                    result.insert(format!("gauge.{}", key), json);
                }
            }
        }

        // Add histograms
        {
            let histograms = try_lock_with_backoff(&*self.histograms).map_err(|_| MetricsError::LockPoisoned)?;
            for (key, histogram) in histograms.iter() {
                if let Ok(json) = serde_json::to_value(histogram) {
                    result.insert(format!("histogram.{}", key), json);
                }
            }
        }

        Ok(result)
    }

    // Helper methods

    /// Get elapsed milliseconds since collector start
    fn elapsed_millis(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn try_lock_with_backoff<T>(lock: &Mutex<T>) -> Result<MutexGuard<T>, MetricsError> {
    let mut attempts = 0;
    loop {
        if let Ok(guard) = lock.try_lock() {
            return Ok(guard);
        }
        attempts += 1;
        let delay = Duration::from_millis(10 * attempts as u64);
        std::thread::sleep(delay);
        if attempts > 10 {
            return Err(MetricsError::LockTimeout);
        }
    }
}

#[derive(Debug)]
pub enum MetricsError {
    LockPoisoned,
    LockTimeout,
}