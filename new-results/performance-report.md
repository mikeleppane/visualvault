# Performance Report - $(date)

## Environment
- **OS**: Linux
- **CPU**: $(lscpu | grep "Model name" | cut -d':' -f2 | xargs)
- **Rust Version**: $(rustc --version)
- **Commit**: 5000808e4cf75afeb70653c5d6657667405f9f27
- **Branch**: main

## Results
```
test duplicate_detection/1000 ... bench: 3 ns/iter (+/- 0)
test duplicate_detection/5000 ... bench: 3 ns/iter (+/- 0)
test duplicate_detection/10000 ... bench: 3 ns/iter (+/- 0)
test duplicate_detection/1000 #2 ... bench: 3 ns/iter (+/- 0)
test duplicate_detection/5000 #2 ... bench: 3 ns/iter (+/- 0)
test duplicate_detection/10000 #2 ... bench: 3 ns/iter (+/- 0)
test duplicate_ratios/10% ... bench: 3 ns/iter (+/- 0)
test duplicate_ratios/30% ... bench: 3 ns/iter (+/- 0)
test duplicate_ratios/50% ... bench: 3 ns/iter (+/- 0)
test duplicate_ratios/70% ... bench: 3 ns/iter (+/- 0)
test organize_by_type/100 ... bench: 5244836 ns/iter (+/- 121376)
test organize_by_type/500 ... bench: 27964946 ns/iter (+/- 1260819)
test organize_by_type/1000 ... bench: 57821791 ns/iter (+/- 2100009)
test organize_modes/yearly ... bench: 58079757 ns/iter (+/- 1451213)
test organize_modes/monthly ... bench: 60238287 ns/iter (+/- 2311774)
test organize_modes/type ... bench: 57675975 ns/iter (+/- 1966807)
test scanner/100 ... bench: 2728575 ns/iter (+/- 42269)
test scanner/1000 ... bench: 25161867 ns/iter (+/- 594615)
test scanner/5000 ... bench: 125318043 ns/iter (+/- 5114516)
test scanner_parallel/1 ... bench: 143128048 ns/iter (+/- 3672138)
test scanner_parallel/2 ... bench: 44569399 ns/iter (+/- 427471)
test scanner_parallel/4 ... bench: 41698581 ns/iter (+/- 777213)
test scanner_parallel/8 ... bench: 39052384 ns/iter (+/- 730567)
```
## Detailed Statistics
### estimates
```json
{
  "mean": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": -0.0302300851644456,
      "upper_bound": -0.010984173980199192
    },
    "point_estimate": -0.020911552454959126,
    "standard_error": 0.004944743100740806
  },
  "median": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": -0.03718582875638188,
      "upper_bound": -0.009770272754433096
    },
    "point_estimate": -0.022936893003273062,
    "standard_error": 0.007448793751833638
  }
}
```
### sample
```json
{
  "sampling_mode": "Linear",
  "iters": [
    18.0,
    36.0,
    54.0,
    72.0,
    90.0,
    108.0,
    126.0,
    144.0,
    162.0,
    180.0
  ],
  "times": [
    47993227.0,
    95945525.0,
    143337334.0,
    191291398.0,
    238898027.0,
```
### benchmark
```json
{
  "group_id": "scanner",
  "function_id": null,
  "value_str": "100",
  "throughput": null,
  "full_id": "scanner/100",
  "directory_name": "scanner/100",
  "title": "scanner/100"
}
```
### estimates
```json
{
  "mean": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": 2660688.2656203704,
      "upper_bound": 2681497.622365521
    },
    "point_estimate": 2669252.619111552,
    "standard_error": 5504.805559156115
  },
  "median": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": 2656824.972222222,
      "upper_bound": 2671552.703703704
    },
    "point_estimate": 2665990.34375,
    "standard_error": 3432.4760422734757
  },
  "median_abs_dev": {
```
### tukey
```json
[
  2623860.918055555,
  2641384.0076388884,
  2688112.246527778,
  2705635.3361111116
]
```
