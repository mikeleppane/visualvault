# Performance Report - $(date)

## Environment
- **OS**: Linux
- **CPU**: $(lscpu | grep "Model name" | cut -d':' -f2 | xargs)
- **Rust Version**: $(rustc --version)
- **Commit**: ad6fc5934142599e54b3111abc85b4da62bf9400
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
test organize_by_type/100 ... bench: 5353252 ns/iter (+/- 195181)
test organize_by_type/500 ... bench: 28619936 ns/iter (+/- 662086)
test organize_by_type/1000 ... bench: 57579158 ns/iter (+/- 2085938)
test organize_modes/yearly ... bench: 57301414 ns/iter (+/- 1836371)
test organize_modes/monthly ... bench: 60034918 ns/iter (+/- 1561974)
test organize_modes/type ... bench: 58911164 ns/iter (+/- 1501464)
test scanner/100 ... bench: 2678426 ns/iter (+/- 26023)
test scanner/1000 ... bench: 24185918 ns/iter (+/- 281502)
test scanner/5000 ... bench: 123452427 ns/iter (+/- 1442588)
test scanner_parallel/1 ... bench: 140518475 ns/iter (+/- 2674006)
test scanner_parallel/2 ... bench: 43026540 ns/iter (+/- 258004)
test scanner_parallel/4 ... bench: 38896128 ns/iter (+/- 478705)
test scanner_parallel/8 ... bench: 36954140 ns/iter (+/- 333855)
```
## Detailed Statistics
### estimates
```json
{
  "mean": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": -0.01700677408478755,
      "upper_bound": -0.0024373449855314435
    },
    "point_estimate": -0.009680445215821676,
    "standard_error": 0.003728920628035764
  },
  "median": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": -0.01913950135716469,
      "upper_bound": 0.0033465749195848993
    },
    "point_estimate": -0.010067598999485727,
    "standard_error": 0.005648773304590443
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
    47537944.0,
    96010312.0,
    141640230.0,
    188481224.0,
    237160411.0,
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
      "lower_bound": 2637724.4681481486,
      "upper_bound": 2660648.572325838
    },
    "point_estimate": 2649513.785936949,
    "standard_error": 5872.826427299951
  },
  "median": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": 2631982.0555555555,
      "upper_bound": 2668259.7685185187
    },
    "point_estimate": 2651460.985008818,
    "standard_error": 9983.20318399398
  },
  "median_abs_dev": {
```
### tukey
```json
[
  2542544.609722222,
  2589565.295138889,
  2714953.789583334,
  2761974.4750000006
]
```
