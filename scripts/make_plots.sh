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
    echo "Usage: $0 {MLTL|STL} [--timeout N] [--bench-sets \"SET1 SET2 ...\"] [--base-dir DIR] [--output-dir DIR] [--adjacent-plots]"
    exit 1
fi


basedir=""
timeout=120
datasets=()
outdir="../resources/results/plots"
adjacent_plots=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --timeout)
            timeout="$2"
            shift 2
            ;;
        --bench-sets)
            datasets=($2)
            shift 2
            ;;
        --base-dir)
            basedir="$2"
            shift 2
            ;;
        --output-dir)
            outdir="$2"
            shift 2
            ;;
        --adjacent-plots)
            adjacent_plots=true
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            exit 1
            ;;
    esac
done

# Set defaults based on logic
if [ "$logic" = "MLTL" ]; then
    if [ -z "$basedir" ]; then
        basedir="../resources/results/MLTL"
    fi
    if [ ${#datasets[@]} -eq 0 ]; then
        datasets=("nasa-boeing" "random" "random0")
    fi
    tools="STLSat parallel,STLSat tableau,STLSat FOL,STLSat SMT,STLTree unsound tableau,MLTLSAT (Z3 4.15.3)"
    tool_names=("stlsat_parallel" "stlsat" "stlsat_fol" "stlsat_smt" "stltree" "mltlsat")
    prefix="mltl"
elif [ "$logic" = "STL" ]; then
    if [ -z "$basedir" ]; then
        basedir="../resources/results/STL"
    fi
    if [ ${#datasets[@]} -eq 0 ]; then
        datasets=("random" "random0")
    fi
    tools="STLSat parallel,STLSat tableau,STLSat DL tableau,STLSat FOL,STLSat SMT,STLTree unsound tableau"
    tool_names=("stlsat_parallel" "stlsat" "stlsat_dl" "stlsat_fol" "stlsat_smt" "stltree")
    prefix="stl"
fi

if [ ! -d "${outdir}" ]; then
    mkdir -p "${outdir}"
fi

set -x

# Generate main plots
y_label=
plot_no=0
for dataset in "${datasets[@]}"; do
    if ((plot_no > 0)) && [ "$adjacent_plots" = true ]; then
        y_label="--no-y-label"
    fi
    ((plot_no++))
    python3 plot.py "${tools}" "$(make_tools_csvs "${basedir}" "${dataset}" "${tool_names[@]}")" ${timeout} --survival --markers-survival ${y_label} -o "${outdir}/${prefix}_${dataset}"
done


# Generate scatter plots
tools_scatter="STLSat (tableau),STLSat (FOL)"
tool_names_scatter=("stlsat" "stlsat_fol")


y_label=
plot_no=0
for dataset in "${datasets[@]}"; do
    if ((plot_no > 0)) && [ "$adjacent_plots" = true ]; then
        y_label="--no-y-label"
    fi
    ((plot_no++))
    python3 plot.py "${tools_scatter}" "$(make_tools_csvs "${basedir}" "${dataset}" "${tool_names_scatter[@]}")" ${timeout} --scatter ${y_label} -o "${outdir}/${prefix}_${dataset}"
done
