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
    echo "Usage: $0 {MLTL|STL} [--timeout N] [--bench-sets \"SET1 SET2 ...\"] [--base-dir DIR] [--output-dir DIR] [--adjacent-plots] [--survival-tools TOOL1,TOOL2,...] [--survival-labels LABEL1,LABEL2] [--scatter-tools TOOL1,TOOL2,...]"
    exit 1
fi


basedir=""
timeout=120
datasets=()
outdir="../resources/results/plots"
adjacent_plots=false
survival_tools=()
survival_labels=()
scatter_tools=()

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
        --survival-tools)
            IFS=',' read -r -a survival_tools <<< "$2"
            shift 2
            ;;
        --survival-labels)
            IFS=',' read -r -a survival_labels <<< "$2"
            shift 2
            ;;
        --scatter-tools)
            IFS=',' read -r -a scatter_tools <<< "$2"
            shift 2
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
    default_survival_tools=("stlsat_parallel" "stlsat" "stlsat_no_jump" "stlsat_fol" "stlsat_smt" "stltree" "mltlsat")
    default_survival_labels=("STLSat parallel" "STLSat JUMP tableau" "STLSat basic tableau" "STLSat FOL" "STLSat SMT" "STLTree unsound tableau" "MLTLSAT")
    prefix="mltl"
elif [ "$logic" = "STL" ]; then
    if [ -z "$basedir" ]; then
        basedir="../resources/results/STL"
    fi
    if [ ${#datasets[@]} -eq 0 ]; then
        datasets=("random" "random0")
    fi
    default_survival_tools=("stlsat_parallel" "stlsat" "stlsat_no_jump" "stlsat_fol" "stlsat_smt" "stltree")
    default_survival_labels=("STLSat parallel" "STLSat JUMP tableau" "STLSat basic tableau" "STLSat FOL" "STLSat SMT" "STLTree unsound tableau")
    prefix="stl"
fi

if [ ${#survival_tools[@]} -eq 0 ]; then
    survival_tools=("${default_survival_tools[@]}")
    survival_labels=("${default_survival_labels[@]}")
fi

if [ ${#survival_labels[@]} -eq 0 ]; then
    survival_labels=("${survival_tools[@]}")
fi
if [ ${#survival_tools[@]} -lt 2 ]; then
    echo "Error: survival tools must include at least two tools"
    exit 1
fi

function get_survival_label() {
    local requested_tool="$1"
    local i

    for i in "${!survival_tools[@]}"; do
        if [ "${survival_tools[$i]}" = "$requested_tool" ]; then
            echo "${survival_labels[$i]}"
            return 0
        fi
    done

    echo "Error: scatter tool '$requested_tool' has no corresponding survival label" >&2
    return 1
}

if [ ! -d "${outdir}" ]; then
    mkdir -p "${outdir}"
fi

set -x

# Generate main plots
survival_tool_list=$(printf '%s,' "${survival_labels[@]}")
survival_tool_list="${survival_tool_list%,}"

y_label=
plot_no=0
for dataset in "${datasets[@]}"; do
    if ((plot_no > 0)) && [ "$adjacent_plots" = true ]; then
        y_label="--no-y-label"
    fi
    ((plot_no++))
    python3 plot.py "${survival_tool_list}" "$(make_tools_csvs "${basedir}" "${dataset}" "${survival_tools[@]}")" ${timeout} --survival --markers-survival ${y_label} -o "${outdir}/${prefix}_${dataset}"
done


scatter_plot_tools=()
scatter_plot_labels=()

if [ ${#scatter_tools[@]} -gt 0 ]; then
    if (( ${#scatter_tools[@]} % 2 != 0 )); then
        echo "Error: --scatter-tools must specify an even number of tool names separated by commas"
        exit 1
    fi

    for tool in "${scatter_tools[@]}"; do
        if ! label=$(get_survival_label "$tool"); then
            echo "Error: scatter tool '$tool' has no corresponding survival label" >&2
            exit 1
        fi
        scatter_plot_tools+=("$tool")
        scatter_plot_labels+=("$label")
    done
else
    default_scatter_pairs=(
        "stlsat|stlsat_fol"
        "stlsat|stlsat_smt"
    )
    generated_default_scatter=false

    for pair in "${default_scatter_pairs[@]}"; do
        IFS='|' read -r -a pair_parts <<< "$pair"
        tool_a="${pair_parts[0]}"
        tool_b="${pair_parts[1]}"

        if printf '%s\n' "${survival_tools[@]}" | grep -Fxq "$tool_a" && printf '%s\n' "${survival_tools[@]}" | grep -Fxq "$tool_b"; then
            scatter_plot_tools+=("$tool_a" "$tool_b")
            scatter_plot_labels+=("$(get_survival_label "$tool_a")" "$(get_survival_label "$tool_b")")
            generated_default_scatter=true
        fi
    done

    if [ "$generated_default_scatter" = false ]; then
        scatter_plot_tools+=("${survival_tools[0]}" "${survival_tools[1]}")
        scatter_plot_labels+=("${survival_labels[0]}" "${survival_labels[1]}")
    fi
fi

for ((pair_index = 0; pair_index < ${#scatter_plot_tools[@]}; pair_index += 2)); do
    tool_a="${scatter_plot_tools[$pair_index]}"
    tool_b="${scatter_plot_tools[$((pair_index + 1))]}"
    label_a="${scatter_plot_labels[$pair_index]}"
    label_b="${scatter_plot_labels[$((pair_index + 1))]}"

    y_label=
    plot_no=0
    for dataset in "${datasets[@]}"; do
        if ((plot_no > 0)) && [ "$adjacent_plots" = true ]; then
            y_label="--no-y-label"
        fi
        ((plot_no++))
        python3 plot.py "${label_a},${label_b}" "$(make_tools_csvs "${basedir}" "${dataset}" "$tool_a" "$tool_b")" ${timeout} --scatter ${y_label} -o "${outdir}/${prefix}_${tool_a}_vs_${tool_b}_${dataset}"
    done
done
