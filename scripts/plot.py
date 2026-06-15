#!/usr/bin/env python3

import os, os.path, sys, argparse, subprocess, shutil, importlib
import plotly.graph_objects as go
import pandas as pd
import math

def valid_result(r):
    return r in {'sat', 'unsat'}

def merge_results(row):
    r1 = row['Result_1']
    r2 = row['Result_2']
    result = None
    if valid_result(r1):
        result = r1
    if valid_result(r2):
        if result is None:
            result = r2
        elif result != r2:
            print(f"WARNING: Inconsistent results: {str(row)}")
    
    if result is None:
        return 'unknown'
    return result

def read_csv_files(tools, csv_files, timeout):
    """
    Reads the CSV files and returns a dictionary with the tools as keys
    and their corresponding data as values.
    """
    data = {}
    for tool, csv_file in zip(tools, csv_files):
        if not os.path.exists(csv_file):
            print(f"Warning: {csv_file} does not exist. Skipping {tool}.")
            continue
        df = pd.read_csv(csv_file)
        df.loc[(df["Time (s)"] == -1) | (~ df["Result"].map(valid_result)), "Time (s)"] = timeout
        data[tool] = df
    return data

def make_survival_line(tool, data, markers):
    """
    Creates a survival line for the given tool and its data.
    """
    # Sort the data by time
    data = data.sort_values(by="Time (s)", ascending=True, ignore_index=True)

    # Create the line
    return go.Scatter(
        x=data["Time (s)"],
        y=data.index,
        mode='lines+markers' if markers else 'lines',
        name=tool,
        marker=dict(size=4, symbol='x'),
        line=dict(shape='linear',width=2,simplify=True)
    ) 

def write_tight_pdf(fig, output_pdf, size):
    """
    Writes a tight PDF by exporting SVG first, then converting to PDF.
    Falls back to Plotly's PDF export if no converter is available.
    """
    tmp_svg = output_pdf + ".tmp.svg"
    fig.write_image(tmp_svg, format='svg', width=size, height=size)

    converted = False

    rsvg_convert = shutil.which("rsvg-convert")
    if rsvg_convert:
        result = subprocess.run(
            [rsvg_convert, "-f", "pdf", "-o", output_pdf, tmp_svg],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0:
            converted = True
        else:
            print(f"Warning: rsvg-convert failed, falling back ({result.stderr.strip()})")

    if not converted:
        try:
            cairosvg = importlib.import_module("cairosvg")
            cairosvg.svg2pdf(url=tmp_svg, write_to=output_pdf)
            converted = True
        except Exception as exc:
            print(f"Warning: SVG->PDF conversion unavailable, using Plotly PDF export ({exc})")

    if not converted:
        fig.write_image(output_pdf, format='pdf', width=size, height=size)

    if os.path.exists(tmp_svg):
        os.remove(tmp_svg)


def make_survival_plot(tools, data, output, timeout, markers, size, no_y_label, log_scale, no_legend):
    # Create the Plotly object for Figure
    fig = go.Figure()

    for tool in tools:
        line = make_survival_line(tool, data[tool], markers)
        fig.add_trace(line)

    # set the labels
    fig.update_layout(
        font_family="Linux Libertine Display O,serif",
        #font_size=12,
        xaxis_title="Time (s)",
        yaxis_title=None if no_y_label else "Number of benchmarks solved",
        margin=dict(l=0, r=0, t=0, b=0, pad=0),
        plot_bgcolor='white',      # No background color
        paper_bgcolor='white',     # No outer background
        xaxis=dict(
            type='log' if log_scale else 'linear',
            range=(-2, math.log10(timeout)+.01) if log_scale else (0, timeout),
            title_standoff=5,
            automargin=True,
            showgrid=True,
            gridcolor='lightgrey',  # Grey grid lines
            # showline=True,             # Draw x=0 axis
            # linecolor='black',     # Axis color
            # linewidth=1
            # mirror=True
        ),
        yaxis=dict(
            title_standoff=5,
            automargin=True,
            showgrid=True,
            gridcolor='lightgrey',  # Grey grid lines
            showline=True,             # Draw x=0 axis
            linecolor='black',     # Axis color
            linewidth=1,
            mirror=True,
            zeroline=True,
            zerolinecolor='black',  # Color of the zero line
            zerolinewidth=1,         # Width of the zero line
        ),
        showlegend=not no_legend,
        legend=dict(
            yanchor="bottom",
            y=0.05,
            xanchor="right",
            x=0.99,
            itemwidth=30,
            tracegroupgap=0,
            indentation=0,
            bgcolor="rgba(0,0,0,0)",
            font=dict(size=10),
        )
    )

    write_tight_pdf(fig, output+".survival.pdf", size)


def plot_identity_line(fig, end):
    idline = [0, end+.01]
    fig.add_trace(go.Scatter(
        x=idline,
        y=idline,
        mode='lines+markers',
        line=dict(
            color="black",
            width=1,
        )
    ))

def make_scatter_plot(data, output, timeout, size, no_y_label):
    """
    Creates a scatter plot for the given data.
    """
    (tool1, data1), (tool2, data2) = tuple(data.items())
    joint_data = data1.merge(data2, on="Name", suffixes=("_1", "_2"), validate="one_to_one")
    # print(joint_data)

    result_summary = joint_data.apply(merge_results, axis=1, result_type='expand')
    joint_data["Result"] = result_summary

    sat_points = joint_data[joint_data["Result"] == "sat"]
    unsat_points = joint_data[joint_data["Result"] == "unsat"]
    unknown_points = joint_data[joint_data["Result"] == "unknown"]

    fig = go.Figure()
    plot_identity_line(fig, timeout)

    fig.add_trace(go.Scatter(
        x=sat_points["Time (s)_1"],
        y=sat_points["Time (s)_2"],
        mode='markers',
        marker=dict(size=5, symbol='x', color='green'),
        cliponaxis=False,
    ))

    fig.add_trace(go.Scatter(
        x=unsat_points["Time (s)_1"],
        y=unsat_points["Time (s)_2"],
        mode='markers',
        marker=dict(size=5, symbol='x', color='red'),
        cliponaxis=False,
    ))

    fig.add_trace(go.Scatter(
        x=unknown_points["Time (s)_1"],
        y=unknown_points["Time (s)_2"],
        mode='markers',
        marker=dict(size=5, symbol='x', color='gray'),
        cliponaxis=False,
    ))

    # Set the layout
    fig.update_layout(
        #title="Scatter Plot of Benchmark Results",
        font_family="Linux Libertine Display O,serif",
        showlegend=False,
        xaxis_title=tool1 + " time (s)",
        yaxis_title=None if no_y_label else tool2 + " time (s)",
        margin=dict(l=0, r=0, t=0, b=0, pad=0),
        plot_bgcolor='white',
        paper_bgcolor='white',
        xaxis=dict(
            type='log',
            range=(-2, math.log10(timeout)+.01),
            title_standoff=5,
            automargin=True,
            showgrid=True,
            gridcolor='lightgrey',
            layer='below traces',
            showline=True,
            linecolor='black',
            linewidth=1,
            mirror=True,
        ),
        yaxis=dict(
            type='log',
            range=(-2, math.log10(timeout)+.01),
            title_standoff=5,
            automargin=True,
            # scaleanchor = "x",
            # scaleratio = 1,
            showgrid=True,
            gridcolor='lightgrey',
            layer='below traces',
            showline=True,
            linecolor='black',
            linewidth=1,
            mirror=True
        ),
    )

    write_tight_pdf(fig, output+".scatter.pdf", size)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument('tools',
                        help='Comma-separated tool names.')
    parser.add_argument('tool_csvs',
                        help='List of comma-separated CSV files containing tool data.')
    parser.add_argument('timeout', type=int,
                        help='Time value for the timeout.')
    parser.add_argument('-o', '--output', dest='output', 
                        default='output',
                        help='Name of the output file')
    parser.add_argument('--survival', action='store_true',
                        help='Create a survival plot (default).')
    parser.add_argument('--scatter', action='store_true',
                        help='Create a scatter plot.')
    parser.add_argument('--no-legend', action='store_true',
                        help='Do not show the legend in the plot.')
    parser.add_argument('--log-survival', action='store_true',
                        help='Use a log scale for the time axis in survival plots.')
    parser.add_argument('--markers-survival', action='store_true',
                        help='Use markers in the survival plot.')
    parser.add_argument('--no-y-label', action='store_true',
                        help='Do not show the y axis label.')
    parser.add_argument('--size', type=int, default=300,
                        help='Size of the output plot in pixels (both width and height).')
    args = parser.parse_args()
    
    if args.timeout < 0:
        sys.exit('Please specify a positive timeout value.')
    
    tools = args.tools.strip().split(",")
    tool_csvs = args.tool_csvs.strip().split(",")
    if len(tools) != len(tool_csvs):
        sys.exit("Error: different numbers of tools and CSV files were entered.")

    data = read_csv_files(tools, tool_csvs, args.timeout)

    if args.scatter:
        if len(tools) == 2:
            make_scatter_plot(data, args.output, args.timeout, args.size, args.no_y_label)
        else:
            print("Scatter plot is only available for exactly two tools.")
    else:
        make_survival_plot(tools, data, args.output, args.timeout, args.markers_survival, args.size, args.no_y_label, args.log_survival, args.no_legend)
