#!/bin/bash

if [ $# -lt 1 ]; then
    echo "Usage: $0 mltlsatdir [--timeout SECONDS] [--jobs N] [--max-mem MB] [--iters N] [--z3bin PATH] [--bench-sets \"SET1 SET2 ...\"] [--tools \"TOOL1 TOOL2 ...\"] [--stltree-path PATH] [--output-dir DIR]"
    exit 1
fi

mltlsatdir="$1"
shift

timeout=120
jobs=4
max_mem=30720
iters=5
z3bin=z3
bench_sets=("nasa-boeing" "random" "random0")
tools=("stlsat" "stlsat_fol" "stlsat_smt" "stlsat_parallel" "mltlsat" "stltree")
outdir=./output_mltl

while [[ $# -gt 0 ]]; do
    case "$1" in
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
        --z3bin)
            z3bin="$2"
            shift 2
            ;;
        --bench-sets)
            bench_sets=($2)
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
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_${bench_set}.csv" -b "${mltlsatdir}/" "${mltlsatdir}/benchmark_list/${bench_set}.list" stlsat --mltl --engine tableau &> "${outdir}/stlsat_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_fol " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_fol_${bench_set}.csv" -b "${mltlsatdir}/" "${mltlsatdir}/benchmark_list/${bench_set}.list" stlsat --mltl --engine fol &> "${outdir}/stlsat_fol_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_smt " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_smt_${bench_set}.csv" -b "${mltlsatdir}/" "${mltlsatdir}/benchmark_list/${bench_set}.list" stlsat --mltl --engine smt &> "${outdir}/stlsat_smt_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stlsat_parallel " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stlsat_parallel_${bench_set}.csv" -b "${mltlsatdir}/" "${mltlsatdir}/benchmark_list/${bench_set}.list" stlsat-parallel --mltl &> "${outdir}/stlsat_parallel_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " mltlsat " ]]; then
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/mltlsat_${bench_set}.csv" -b "${mltlsatdir}/" "${mltlsatdir}/benchmark_list/${bench_set}.list" smt-quant "${mltlsatdir}/translator/src/MLTLConvertor" "${z3bin}" &> "${outdir}/mltlsat_${bench_set}.log"
    done
fi

if [[ " ${tools[@]} " =~ " stltree " ]]; then
    if [ -z "${stltree_path}" ]; then
        echo "Error: --stltree-path must be provided when using stltree tool."
        exit 1
    fi
    for bench_set in "${bench_sets[@]}"; do
        ./run_bench.py --timeout ${timeout} --max-mem ${max_mem} --jobs ${jobs} --iters ${iters} -vv --csv "${outdir}/stltree_${bench_set}.csv" -b "${mltlsatdir}/" "${mltlsatdir}/benchmark_list/${bench_set}.list" stltree "${stltree_path}" --mltl &> "${outdir}/stltree_${bench_set}.log"
    done
fi
