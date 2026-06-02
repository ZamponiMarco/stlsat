#!/bin/bash

if [ $# -lt 1 ]; then
    echo "Usage: $0 [--benchdir DIR] [--timeout SECONDS] [--jobs N] [--max-mem MB] [--iters N] [--tools \"TOOL1 TOOL2 ...\"] [--stltree-path PATH] [--bench-sets \"SET1 SET2 ...\"] [--output-dir DIR]"
    exit 1
fi

benchdir="../resources/benchmarks"
timeout=120
jobs=4
max_mem=30720
iters=5
tools=("stlsat" "stlsat_fol" "stlsat_parallel" "stlsat_dl" "stlsat_nofs" "stlsat_smt" "stltree")
bench_sets=("random" "random0")
outdir=./output_stl

while [[ $# -gt 0 ]]; do
    case "$1" in
        --benchdir)
            benchdir="$2"
            shift 2
            ;;
        --timeout)
            timeout="$2"
            shift 2
            ;;
        --jobs)
            jobs="$2"
            shift 2
            ;;
        --max-mem)
            max_mem="$2"
            shift 2
            ;;
        --iters)
            iters="$2"
            shift 2
            ;;
        --tools)
            tools=($2)
            shift 2
            ;;
        --stltree-path)
            stltree_path="$2"
            shift 2
            ;;
        --bench-sets)
            bench_sets=($2)
            shift 2
            ;;
        --output-dir)
            outdir="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1"
            exit 1
            ;;
    esac
done

if [ ! -d "${outdir}" ]; then
    mkdir -p "${outdir}"
fi

ulimit -s unlimited

set -x

if [[ " ${tools[@]} " =~ " stlsat " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stlsat --engine tableau &> "${outdir}/stlsat_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_fol " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_fol_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stlsat --engine fol &> "${outdir}/stlsat_fol_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_parallel " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_parallel_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stlsat-parallel &> "${outdir}/stlsat_parallel_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_dl " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_dl_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stlsat --engine tableau --solver auto &> "${outdir}/stlsat_dl_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_nofs " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_nofs_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stlsat --no-formula-simplifications --engine tableau &> "${outdir}/stlsat_nofs_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_smt " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_smt_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stlsat --engine smt &> "${outdir}/stlsat_smt_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stltree " ]]; then
    if [ -z "${stltree_path}" ]; then
        echo "Error: --stltree-path must be provided when using stltree tool."
        exit 1
    fi
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stltree_${bench_set}.csv" -b "${benchdir}/${bench_set}/" "${benchdir}/${bench_set}/${bench_set}.list" stltree "${stltree_path}" &> "${outdir}/stltree_${bench_set}.log"
    done
fi
