// Principal values of the complex functions, against Python's `cmath` (C99 cuts).
#![allow(clippy::approx_constant, clippy::type_complexity)]

use omasheet_engine::Value;
use omasheet_engine::omx::funcs::MathFn;
use omasheet_engine::value::math;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * (1.0 + b.abs())
}

#[test]
fn the_complex_functions_match_cmath() {
    let cases: &[(MathFn, (f64, f64), (f64, f64))] = &[
        (
            MathFn::Sin,
            (1.0, 2.0),
            (3.165778513216168, 1.959601041421606),
        ),
        (
            MathFn::Sin,
            (-1.0, -2.0),
            (-3.165778513216168, -1.959601041421606),
        ),
        (
            MathFn::Sin,
            (3.0, 0.5),
            (0.15913058529843999, -0.5158804424525267),
        ),
        (
            MathFn::Cos,
            (1.0, 2.0),
            (2.0327230070196656, -3.0518977991517997),
        ),
        (
            MathFn::Cos,
            (-1.0, -2.0),
            (2.0327230070196656, -3.0518977991517997),
        ),
        (
            MathFn::Tan,
            (1.0, 2.0),
            (0.0338128260798967, 1.0147936161466333),
        ),
        (
            MathFn::Tan,
            (-1.0, -2.0),
            (-0.0338128260798967, -1.0147936161466333),
        ),
        (MathFn::Tan, (0.5, 30.0), (1.473669946993437e-26, 1.0)),
        (
            MathFn::Sinh,
            (1.0, 2.0),
            (-0.4890562590412937, 1.4031192506220405),
        ),
        (
            MathFn::Sinh,
            (-1.0, -2.0),
            (0.4890562590412937, -1.4031192506220405),
        ),
        (
            MathFn::Cosh,
            (1.0, 2.0),
            (-0.64214812471552, 1.0686074213827783),
        ),
        (
            MathFn::Tanh,
            (1.0, 2.0),
            (1.16673625724092, -0.24345820118572534),
        ),
        (
            MathFn::Tanh,
            (-1.0, -2.0),
            (-1.16673625724092, 0.24345820118572534),
        ),
        (
            MathFn::Tanh,
            (0.5, -3.0),
            (0.46946999342588536, 0.11162105077158349),
        ),
        (
            MathFn::Arcsin,
            (2.0, 0.0),
            (1.5707963267948966, 1.3169578969248166),
        ),
        (
            MathFn::Arcsin,
            (-2.0, 0.0),
            (-1.5707963267948966, 1.3169578969248166),
        ),
        (
            MathFn::Arcsin,
            (0.5, 0.5),
            (0.45227844715119064, 0.5306375309525179),
        ),
        (
            MathFn::Arcsin,
            (-0.5, -0.5),
            (-0.45227844715119064, -0.5306375309525179),
        ),
        (
            MathFn::Arcsin,
            (2.0, -0.0),
            (1.5707963267948966, -1.3169578969248166),
        ),
        (MathFn::Arccos, (2.0, 0.0), (0.0, -1.3169578969248166)),
        (
            MathFn::Arccos,
            (-2.0, 0.0),
            (3.141592653589793, -1.3169578969248166),
        ),
        (
            MathFn::Arccos,
            (0.5, 0.5),
            (1.1185178796437059, -0.5306375309525179),
        ),
        (
            MathFn::Arccos,
            (-0.5, -0.5),
            (2.0230747739460875, 0.5306375309525179),
        ),
        (
            MathFn::Arctan,
            (1.0, 2.0),
            (1.3389725222944935, 0.40235947810852507),
        ),
        (
            MathFn::Arctan,
            (-1.0, -2.0),
            (-1.3389725222944935, -0.40235947810852507),
        ),
        (
            MathFn::Arctan,
            (0.0, 2.0),
            (1.5707963267948966, 0.5493061443340549),
        ),
        (
            MathFn::Arctan,
            (0.5, 0.5),
            (0.5535743588970452, 0.40235947810852507),
        ),
        (
            MathFn::Arcsinh,
            (1.0, 2.0),
            (1.4693517443681852, 1.0634400235777521),
        ),
        (
            MathFn::Arcsinh,
            (-1.0, -2.0),
            (-1.4693517443681852, -1.0634400235777521),
        ),
        (
            MathFn::Arcsinh,
            (0.0, 2.0),
            (1.3169578969248166, 1.5707963267948966),
        ),
        (MathFn::Arcsinh, (-3.0, 0.0), (-1.8184464592320668, 0.0)),
        (MathFn::Arccosh, (0.0, 0.0), (0.0, 1.5707963267948966)),
        (
            MathFn::Arccosh,
            (-2.0, 0.0),
            (1.3169578969248166, 3.141592653589793),
        ),
        (
            MathFn::Arccosh,
            (1.0, 2.0),
            (1.5285709194809982, 1.1437177404024204),
        ),
        (MathFn::Arccosh, (0.5, 0.0), (0.0, 1.0471975511965979)),
        (
            MathFn::Arctanh,
            (2.0, 0.0),
            (0.5493061443340549, 1.5707963267948966),
        ),
        (
            MathFn::Arctanh,
            (-2.0, 0.0),
            (-0.5493061443340549, 1.5707963267948966),
        ),
        (
            MathFn::Arctanh,
            (0.5, 0.5),
            (0.40235947810852507, 0.5535743588970452),
        ),
        (
            MathFn::Arctanh,
            (0.5, -0.5),
            (0.40235947810852507, -0.5535743588970452),
        ),
        (MathFn::Ln, (-1.0, 0.0), (0.0, 3.141592653589793)),
        (
            MathFn::Ln,
            (-4.0, 0.0),
            (1.3862943611198906, 3.141592653589793),
        ),
        (
            MathFn::Ln,
            (1.0, 2.0),
            (0.8047189562170503, 1.1071487177940904),
        ),
        (MathFn::Log10, (-100.0, 0.0), (2.0, 1.3643763538418412)),
        (
            MathFn::Log10,
            (1.0, 2.0),
            (0.3494850021680094, 0.480828578784234),
        ),
        (MathFn::Sqrt, (-4.0, 0.0), (0.0, 2.0)),
        (
            MathFn::Sqrt,
            (1.0, 2.0),
            (1.272019649514069, 0.7861513777574233),
        ),
    ];
    for (m, (re, im), (wre, wim)) in cases {
        match math(*m, &Value::Complex(*re, *im)) {
            Ok(Value::Complex(a, b)) => assert!(
                close(a, *wre) && close(b, *wim),
                "{}({re}{im:+}i) = {a}{b:+}i, wanted {wre}{wim:+}i",
                m.name()
            ),
            other => panic!("{}({re}{im:+}i): {other:?}", m.name()),
        }
    }
}

#[test]
fn a_real_beyond_the_domain_is_complex_and_the_edge_is_real() {
    let at = |m: MathFn, x: f64| math(m, &Value::Float(x)).unwrap();
    assert!(matches!(at(MathFn::Sqrt, -4.0), Value::Complex(re, im) if re == 0.0 && im == 2.0));
    assert!(
        matches!(at(MathFn::Ln, -1.0), Value::Complex(re, im) if re == 0.0 && close(im, std::f64::consts::PI))
    );
    assert!(
        matches!(at(MathFn::Arccos, 2.0), Value::Complex(re, im) if re == 0.0 && close(im, -1.3169578969248166))
    );
    assert!(
        matches!(at(MathFn::Arccosh, 0.0), Value::Complex(re, im) if re == 0.0 && close(im, std::f64::consts::FRAC_PI_2))
    );
    assert!(matches!(at(MathFn::Arctanh, 2.0), Value::Complex(..)));
    for (m, x) in [
        (MathFn::Ln, 0.0),
        (MathFn::Sqrt, 0.0),
        (MathFn::Arctanh, 1.0),
        (MathFn::Arcsin, 1.0),
        (MathFn::Arccosh, 1.0),
    ] {
        assert!(matches!(at(m, x), Value::Float(_)), "{} at {x}", m.name());
    }
    assert!(matches!(at(MathFn::Sqrt, f64::NAN), Value::Float(f) if f.is_nan()));
    // The inside of the domain is real.
    assert!(matches!(at(MathFn::Arcsinh, -3.0), Value::Float(_)));
    // The identities.
    for z in [(0.3, 0.4), (-1.2, 0.7), (2.0, -1.5)] {
        let v = |m: MathFn, w: Value| match math(m, &w).unwrap() {
            Value::Complex(a, b) => (a, b),
            other => panic!("{other:?}"),
        };
        let w = Value::Complex(z.0, z.1);
        let (s, c) = (v(MathFn::Sin, w.clone()), v(MathFn::Cos, w.clone()));
        let (re, im) = (
            s.0 * s.0 - s.1 * s.1 + c.0 * c.0 - c.1 * c.1,
            2.0 * (s.0 * s.1 + c.0 * c.1),
        );
        assert!(close(re, 1.0) && close(im, 0.0), "sin^2+cos^2 at {z:?}");
        let back = v(
            MathFn::Exp,
            Value::Complex(v(MathFn::Ln, w.clone()).0, v(MathFn::Ln, w.clone()).1),
        );
        assert!(
            close(back.0, z.0) && close(back.1, z.1),
            "exp(ln z) at {z:?}"
        );
    }
}
