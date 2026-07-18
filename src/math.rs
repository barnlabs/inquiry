use anyhow::{Context, Result, bail};
use fasteval::{Compiler, Evaler};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calculation {
    pub expression: String,
    pub value: f64,
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSummary {
    pub count: usize,
    pub minimum: f64,
    pub maximum: f64,
    pub mean: f64,
    pub median: f64,
    pub first_quartile: f64,
    pub third_quartile: f64,
    pub sample_standard_deviation: Option<f64>,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericalResult {
    pub expression: String,
    pub value: f64,
    pub method: String,
    pub parameters: Vec<(String, f64)>,
    pub caveat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPoint {
    pub x: f64,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub expression: String,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub samples: usize,
    pub points: Vec<GraphPoint>,
    pub method: String,
    pub warnings: Vec<String>,
}

pub fn evaluate(expression: &str) -> Result<Calculation> {
    let expression = checked_expression(expression)?;
    let evaluator = BoundExpression::parse(expression, false)?;
    let value = evaluator.evaluate(None)?;
    finite(value, "calculation result")?;
    Ok(Calculation {
        expression: expression.into(),
        value,
        engine: "fasteval 0.2.4 safe deterministic expression parser".into(),
    })
}

pub fn summarize(values: &[f64]) -> Result<StatisticsSummary> {
    if values.is_empty() {
        bail!("statistics requires at least one value");
    }
    if values.iter().any(|value| !value.is_finite()) {
        bail!("statistics values must all be finite");
    }
    if values.len() > 1_000_000 {
        bail!("statistics accepts at most 1,000,000 values");
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let count = sorted.len();
    let mean = stable_mean(&sorted);
    let sample_standard_deviation = if count >= 2 {
        let sum_squared = sorted
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>();
        Some((sum_squared / (count - 1) as f64).sqrt())
    } else {
        None
    };
    Ok(StatisticsSummary {
        count,
        minimum: sorted[0],
        maximum: sorted[count - 1],
        mean,
        median: quantile_r7(&sorted, 0.5),
        first_quartile: quantile_r7(&sorted, 0.25),
        third_quartile: quantile_r7(&sorted, 0.75),
        sample_standard_deviation,
        method: "Mean uses compensated summation; quartiles use Hyndman-Fan type 7 (R-7); standard deviation uses the sample n-1 denominator.".into(),
    })
}

pub fn differentiate(expression: &str, at: f64, step: Option<f64>) -> Result<NumericalResult> {
    finite(at, "evaluation point")?;
    let expression = checked_expression(expression)?;
    let function = BoundExpression::parse(expression, true)?;
    let step = step.unwrap_or_else(|| f64::EPSILON.sqrt() * at.abs().max(1.0));
    if !step.is_finite() || step <= 0.0 {
        bail!("derivative step must be finite and greater than zero");
    }
    let plus = function.evaluate(Some(at + step))?;
    let minus = function.evaluate(Some(at - step))?;
    finite(plus, "function value above the evaluation point")?;
    finite(minus, "function value below the evaluation point")?;
    let value = (plus - minus) / (2.0 * step);
    finite(value, "derivative estimate")?;
    Ok(NumericalResult {
        expression: expression.into(),
        value,
        method: "second-order central finite difference".into(),
        parameters: vec![("x".into(), at), ("step".into(), step)],
        caveat: "Numerical differentiation is sensitive to discontinuities, noise, scale, and step choice; compare multiple steps for consequential work.".into(),
    })
}

pub fn integrate(
    expression: &str,
    from: f64,
    to: f64,
    intervals: usize,
) -> Result<NumericalResult> {
    finite(from, "integration lower bound")?;
    finite(to, "integration upper bound")?;
    if from == to {
        return Ok(NumericalResult {
            expression: expression.into(),
            value: 0.0,
            method: "composite Simpson's rule".into(),
            parameters: vec![
                ("from".into(), from),
                ("to".into(), to),
                ("intervals".into(), 0.0),
            ],
            caveat: "Equal bounds have zero signed area.".into(),
        });
    }
    if !(2..=1_000_000).contains(&intervals) || !intervals.is_multiple_of(2) {
        bail!("Simpson integration requires an even interval count from 2 to 1,000,000");
    }
    let expression = checked_expression(expression)?;
    let function = BoundExpression::parse(expression, true)?;
    let width = (to - from) / intervals as f64;
    let mut sum = function.evaluate(Some(from))? + function.evaluate(Some(to))?;
    finite(sum, "integration endpoint values")?;
    for index in 1..intervals {
        let value = function.evaluate(Some(from + index as f64 * width))?;
        finite(value, "integrand sample")?;
        sum += if index % 2 == 0 {
            2.0 * value
        } else {
            4.0 * value
        };
    }
    let value = sum * width / 3.0;
    finite(value, "integral estimate")?;
    Ok(NumericalResult {
        expression: expression.into(),
        value,
        method: "composite Simpson's rule".into(),
        parameters: vec![
            ("from".into(), from),
            ("to".into(), to),
            ("intervals".into(), intervals as f64),
        ],
        caveat: "The estimate can be wrong near discontinuities, singularities, rapid oscillation, or insufficient resolution; inspect the graph and convergence.".into(),
    })
}

pub fn graph(expression: &str, from: f64, to: f64, samples: usize) -> Result<Graph> {
    finite(from, "graph lower bound")?;
    finite(to, "graph upper bound")?;
    if from >= to {
        bail!("graph lower bound must be less than upper bound");
    }
    if !(50..=2_000).contains(&samples) {
        bail!("graph samples must be from 50 to 2,000");
    }
    let expression = checked_expression(expression)?;
    let function = BoundExpression::parse(expression, true)?;
    let width = (to - from) / (samples - 1) as f64;
    let mut points = (0..samples)
        .map(|index| {
            let x = from + index as f64 * width;
            let value = function.evaluate(Some(x)).unwrap_or(f64::NAN);
            GraphPoint {
                x,
                y: value.is_finite().then_some(value),
            }
        })
        .collect::<Vec<_>>();
    let preliminary_values = points
        .iter()
        .filter_map(|point| point.y.map(f64::abs))
        .collect::<Vec<_>>();
    let typical_magnitude = median_value(preliminary_values).max(f64::EPSILON);
    let mut suspected_discontinuities = 0;
    for index in 0..points.len().saturating_sub(1) {
        let (Some(left), Some(right)) = (points[index].y, points[index + 1].y) else {
            continue;
        };
        let midpoint_x = (points[index].x + points[index + 1].x) / 2.0;
        let midpoint = function.evaluate(Some(midpoint_x)).unwrap_or(f64::NAN);
        let sign_flip_near_asymptote = left.is_sign_positive() != right.is_sign_positive()
            && left.abs().min(right.abs()) > typical_magnitude * 10.0;
        let midpoint_spike = midpoint.is_finite()
            && midpoint.abs()
                > (left.abs().max(right.abs()) + typical_magnitude).max(f64::EPSILON) * 8.0;
        if !midpoint.is_finite() || sign_flip_near_asymptote || midpoint_spike {
            points[index + 1].y = None;
            suspected_discontinuities += 1;
        }
    }
    let finite_values = points
        .iter()
        .filter_map(|point| point.y)
        .collect::<Vec<_>>();
    if finite_values.is_empty() {
        bail!("expression produced no finite points in the requested domain");
    }
    let mut y_min = finite_values.iter().copied().fold(f64::INFINITY, f64::min);
    let mut y_max = finite_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if y_min == y_max {
        let padding = y_min.abs().max(1.0) * 0.1;
        y_min -= padding;
        y_max += padding;
    }
    let missing = points.iter().filter(|point| point.y.is_none()).count();
    let mut warnings = vec![
        "The graph samples a function numerically and does not prove continuity, extrema, roots, asymptotes, or behavior between samples.".into(),
    ];
    if missing > 0 {
        warnings.push(format!(
            "{missing} sample point(s) were non-finite and create visible breaks in the plotted line."
        ));
    }
    if suspected_discontinuities > 0 {
        warnings.push(format!(
            "{suspected_discontinuities} interval(s) were broken because midpoint checks indicated a possible discontinuity or vertical asymptote."
        ));
    }
    Ok(Graph {
        expression: expression.into(),
        x_min: from,
        x_max: to,
        y_min,
        y_max,
        samples,
        points,
        method: "uniform x-domain sampling with midpoint and large sign-flip discontinuity checks; SVG polyline segments break at non-finite or suspicious intervals".into(),
        warnings,
    })
}

pub fn write_graph_html(graph: &Graph, path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .with_context(|| {
            format!(
                "could not create {}; graphs never overwrite existing files",
                path.display()
            )
        })?;
    file.write_all(render_graph_html(graph).as_bytes())?;
    Ok(path.to_path_buf())
}

pub fn render_graph_html(graph: &Graph) -> String {
    const WIDTH: f64 = 960.0;
    const HEIGHT: f64 = 520.0;
    const PAD: f64 = 56.0;
    let map_x =
        |x: f64| PAD + (x - graph.x_min) / (graph.x_max - graph.x_min) * (WIDTH - 2.0 * PAD);
    let map_y = |y: f64| {
        HEIGHT - PAD - (y - graph.y_min) / (graph.y_max - graph.y_min) * (HEIGHT - 2.0 * PAD)
    };
    let mut segments = Vec::new();
    let mut segment = Vec::new();
    for point in &graph.points {
        if let Some(y) = point.y {
            segment.push(format!("{:.2},{:.2}", map_x(point.x), map_y(y)));
        } else if !segment.is_empty() {
            segments.push(std::mem::take(&mut segment));
        }
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    let lines = segments
        .into_iter()
        .map(|points| format!(r#"<polyline points="{}"/>"#, points.join(" ")))
        .collect::<String>();
    let x_axis = if graph.y_min <= 0.0 && graph.y_max >= 0.0 {
        format!(
            r#"<line class="axis" x1="{PAD}" x2="{}" y1="{y:.2}" y2="{y:.2}"/>"#,
            WIDTH - PAD,
            y = map_y(0.0)
        )
    } else {
        String::new()
    };
    let y_axis = if graph.x_min <= 0.0 && graph.x_max >= 0.0 {
        format!(
            r#"<line class="axis" y1="{PAD}" y2="{}" x1="{x:.2}" x2="{x:.2}"/>"#,
            HEIGHT - PAD,
            x = map_x(0.0)
        )
    } else {
        String::new()
    };
    let warnings = graph
        .warnings
        .iter()
        .map(|warning| format!("<li>{}</li>", escape(warning)))
        .collect::<String>();
    let stride = (graph.points.len() / 20).max(1);
    let rows = graph
        .points
        .iter()
        .step_by(stride)
        .map(|point| {
            format!(
                "<tr><td>{:.8}</td><td>{}</td></tr>",
                point.x,
                point
                    .y
                    .map(|value| format!("{value:.8}"))
                    .unwrap_or_else(|| "non-finite".into())
            )
        })
        .collect::<String>();
    let x_min_label = compact_number(graph.x_min);
    let x_max_label = compact_number(graph.x_max);
    let y_min_label = compact_number(graph.y_min);
    let y_max_label = compact_number(graph.y_max);
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark light"><title>{expression} — Inquiry Graph</title><style>:root{{--bg:#07100f;--panel:#111a19;--ink:#f5f3ec;--muted:#9aa8a2;--line:#29413c;--lime:#bdfb65;--cyan:#66e3d4}}*{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:var(--ink);font:15px/1.55 system-ui,sans-serif}}main{{width:min(1100px,calc(100% - 24px));margin:32px auto;display:grid;gap:18px}}section{{background:var(--panel);border:1px solid var(--line);border-radius:20px;padding:clamp(18px,4vw,32px)}}h1{{font-size:clamp(30px,6vw,58px);line-height:1;margin:.2em 0}}code{{color:var(--lime)}}.plot{{overflow-x:auto}}svg{{width:100%;min-width:680px;height:auto}}polyline{{fill:none;stroke:var(--cyan);stroke-width:3;stroke-linejoin:round;stroke-linecap:round}}.axis{{stroke:var(--muted);stroke-width:1}}text{{fill:var(--muted);font:13px ui-monospace,monospace}}table{{width:100%;border-collapse:collapse}}th,td{{padding:10px;border-bottom:1px solid var(--line);text-align:left;font-variant-numeric:tabular-nums}}p,li{{color:var(--muted)}}@media(prefers-color-scheme:light){{:root{{--bg:#eef4f1;--panel:#fff;--ink:#10211d;--muted:#526762;--line:#bdd0ca;--lime:#4d7a17;--cyan:#087d71}}}}@media print{{body{{background:#fff}}section{{break-inside:avoid}}}}</style></head><body><main><section><div>BARNLABS / INQUIRY · DETERMINISTIC GRAPH</div><h1><code>{expression}</code></h1><p>x ∈ [{x_min}, {x_max}] · y shown ∈ [{y_min}, {y_max}] · {samples} samples</p></section><section class="plot"><svg viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="Graph of {expression}">{x_axis}{y_axis}{lines}<text x="{PAD}" y="{label_y}">{x_min}</text><text x="{right_label}" y="{label_y}">{x_max}</text><text x="8" y="{PAD}">{y_max}</text><text x="8" y="{bottom_label}">{y_min}</text></svg></section><section><h2>Method and limits</h2><p>{method}</p><ul>{warnings}</ul></section><section><h2>Sample table</h2><table><thead><tr><th>x</th><th>f(x)</th></tr></thead><tbody>{rows}</tbody></table></section></main></body></html>"#,
        expression = escape(&graph.expression),
        x_min = x_min_label,
        x_max = x_max_label,
        y_min = y_min_label,
        y_max = y_max_label,
        samples = graph.samples,
        method = escape(&graph.method),
        warnings = warnings,
        WIDTH = WIDTH,
        HEIGHT = HEIGHT,
        x_axis = x_axis,
        y_axis = y_axis,
        lines = lines,
        PAD = PAD,
        label_y = HEIGHT - 18.0,
        right_label = WIDTH - PAD - 80.0,
        bottom_label = HEIGHT - PAD,
        rows = rows
    )
}

pub fn default_graph_path(expression: &str) -> PathBuf {
    let slug = expression
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "function" } else { &slug };
    PathBuf::from("reports").join(format!(
        "graph-{slug}-{}.html",
        uuid::Uuid::new_v4().simple()
    ))
}

fn checked_expression(expression: &str) -> Result<&str> {
    let expression = expression.trim();
    if expression.is_empty() {
        bail!("expression cannot be empty");
    }
    if expression.chars().count() > 1_000 {
        bail!("expression exceeds the 1,000-character limit");
    }
    Ok(expression)
}

struct BoundExpression {
    slab: fasteval::Slab,
    instruction: fasteval::Instruction,
    allow_x: bool,
}

impl BoundExpression {
    fn parse(expression: &str, allow_x: bool) -> Result<Self> {
        if expression.contains('"') || expression.contains('\'') {
            bail!("string literals are not supported in mathematical expressions");
        }
        let lowered = expression.to_lowercase();
        for blocked in ["print", "let", "push", "pop", "unsafe"] {
            if lowered
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .any(|token| token == blocked)
            {
                bail!("unsupported expression feature: {blocked}");
            }
        }
        let parser = fasteval::Parser::new();
        let mut slab = fasteval::Slab::new();
        let expression_index = parser
            .parse(expression, &mut slab.ps)
            .context("expression could not be parsed")?;
        let expression_ref = expression_index.from(&slab.ps);
        let variables = expression_ref.var_names(&slab);
        let allowed = if allow_x {
            ["x", "pi", "e", "sqrt", "ln", "log10", "exp"].as_slice()
        } else {
            ["pi", "e", "sqrt", "ln", "log10", "exp"].as_slice()
        };
        if let Some(variable) = variables
            .iter()
            .find(|variable| !allowed.contains(&variable.as_str()))
        {
            bail!("unsupported variable '{variable}'; only x, pi, and e are available");
        }
        let instruction = expression_ref.compile(&slab.ps, &mut slab.cs);
        Ok(Self {
            slab,
            instruction,
            allow_x,
        })
    }

    fn evaluate(&self, x: Option<f64>) -> Result<f64> {
        let x = if self.allow_x {
            Some(x.context("expression requires an x value")?)
        } else {
            None
        };
        let mut namespace = |name: &str, arguments: Vec<f64>| -> Option<f64> {
            match (name, arguments.as_slice()) {
                ("pi", []) => Some(std::f64::consts::PI),
                ("e", []) => Some(std::f64::consts::E),
                ("x", []) => x,
                ("sqrt", [value]) => Some(value.sqrt()),
                ("ln", [value]) => Some(value.ln()),
                ("log10", [value]) => Some(value.log10()),
                ("exp", [value]) => Some(value.exp()),
                _ => None,
            }
        };
        self.instruction
            .eval(&self.slab, &mut namespace)
            .context("expression evaluation failed")
    }
}

fn finite(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() {
        bail!("{label} must be finite");
    }
    Ok(())
}

fn stable_mean(values: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for value in values {
        let next = sum + value;
        if sum.abs() >= value.abs() {
            compensation += (sum - next) + value;
        } else {
            compensation += (value - next) + sum;
        }
        sum = next;
    }
    (sum + compensation) / values.len() as f64
}

fn median_value(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    quantile_r7(&values, 0.5)
}

fn quantile_r7(sorted: &[f64], probability: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let index = (sorted.len() - 1) as f64 * probability;
    let lower = index.floor() as usize;
    let fraction = index - lower as f64;
    sorted[lower] + fraction * (sorted[(lower + 1).min(sorted.len() - 1)] - sorted[lower])
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn compact_number(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude >= 1_000_000.0 || (magnitude > 0.0 && magnitude < 0.0001) {
        return format!("{value:.4e}");
    }
    let rendered = format!("{value:.6}");
    let trimmed = rendered.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" {
        "0".into()
    } else {
        trimmed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_and_summarizes_known_values() {
        assert!((evaluate("sin(pi / 2) + 2^3").unwrap().value - 9.0).abs() < 1e-12);
        assert!((evaluate("sqrt(2)^2 + sin(pi/2)").unwrap().value - 3.0).abs() < 1e-12);
        assert!((evaluate("ln(e)").unwrap().value - 1.0).abs() < 1e-12);
        let summary = summarize(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert_eq!(summary.mean, 2.5);
        assert_eq!(summary.median, 2.5);
        assert_eq!(summary.first_quartile, 1.75);
        let cancellation = summarize(&[1e16, 1.0, -1e16]).unwrap();
        assert!((cancellation.mean - 1.0 / 3.0).abs() < 1e-15);
    }

    #[test]
    fn graph_labels_are_compact_but_precise_enough_to_read() {
        assert_eq!(compact_number(12.566370614359172), "12.566371");
        assert_eq!(compact_number(-0.0), "0");
        assert_eq!(compact_number(0.000_001_2), "1.2000e-6");
    }

    #[test]
    fn numerical_calculus_matches_known_results() {
        assert!((differentiate("x^2", 3.0, Some(1e-5)).unwrap().value - 6.0).abs() < 1e-6);
        assert!(
            (integrate("sin(x)", 0.0, std::f64::consts::PI, 1_000)
                .unwrap()
                .value
                - 2.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn graph_breaks_at_non_finite_points_and_escapes_titles() {
        let sampled_pole = graph("1 / x", -1.0, 1.0, 101).unwrap();
        assert!(sampled_pole.points.iter().any(|point| point.y.is_none()));
        let custom = Graph {
            expression: "<script>alert(1)</script>".into(),
            ..sampled_pole
        };
        let html = render_graph_html(&custom);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));

        let unsampled_pole = graph("1/x", -1.0, 1.0, 50).unwrap();
        assert!(unsampled_pole.points.iter().any(|point| point.y.is_none()));
        assert!(
            unsampled_pole
                .warnings
                .iter()
                .any(|warning| warning.contains("possible discontinuity"))
        );
        let tangent = graph("tan(x)", -2.0, 2.0, 401).unwrap();
        assert!(
            tangent
                .points
                .iter()
                .filter(|point| point.y.is_none())
                .count()
                >= 2
        );
    }
}
