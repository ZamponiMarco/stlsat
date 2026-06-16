#!/bin/bash

function make_tools_csvs() {
    set +x
    local basedir="$1"
    shift
    local dataset="$1"
    shift
    local tool_names=("$@")
    local tool_csvs=""

    for tool in "${tool_names[@]}"; do
        tool_csvs+="${basedir}/${tool}_${dataset}.csv,"
    done

    echo "${tool_csvs%,}"
    set -x
}

logic="$1"
shift
if [ "$logic" != "MLTL" ] && [ "$logic" != "STL" ]; then
    echo "Error: first argument must be either MLTL or STL"
    echo "Usage: $0 {MLTL|STL} [--timeout N] [--bench-sets \"SET1 SET2 ...\"] [--base-dir DIR] [--output-dir DIR] [--verdicts \"sat,unsat\"]"
    exit 1
fi

basedir=""
timeout=120
datasets=()
outdir="../resources/results/plots"
verdicts="sat,unsat"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --timeout)    timeout="$2";  shift 2 ;;
        --bench-sets) datasets=($2); shift 2 ;;
        --base-dir)   basedir="$2";  shift 2 ;;
        --output-dir) outdir="$2";   shift 2 ;;
        --verdicts)   verdicts="$2"; shift 2 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

if [ "$logic" = "MLTL" ]; then
    [ -z "$basedir" ] && basedir="../resources/results/MLTL"
    [ ${#datasets[@]} -eq 0 ] && datasets=("nasa-boeing" "random" "random0")
    prefix="mltl"
elif [ "$logic" = "STL" ]; then
    [ -z "$basedir" ] && basedir="../resources/results/STL"
    [ ${#datasets[@]} -eq 0 ] && datasets=("random" "random0")
    prefix="stl"
fi

tool_names=("stlsat" "stlsat_fol" "stlsat_smt")
tools="STLSat tableau,STLSat FOL,STLSat SMT"

[ ! -d "${outdir}" ] && mkdir -p "${outdir}"

# Build one --dataset NAME CSVS argument per benchmark set.
dataset_args=()
for dataset in "${datasets[@]}"; do
    csvs="$(make_tools_csvs "${basedir}" "${dataset}" "${tool_names[@]}")"
    dataset_args+=(--dataset "${dataset}" "${csvs}")
done

set -x

python3 solving_engine_wins.py \
    --tools "${tools}" \
    --timeout "${timeout}" \
    --verdicts "${verdicts}" \
    --caption "${logic}: strict wins per benchmark." \
    --label "tab:${prefix}_wins" \
    -o "${outdir}/${prefix}_wins.tex" \
    "${dataset_args[@]}"