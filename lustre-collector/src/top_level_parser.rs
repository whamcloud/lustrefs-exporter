// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    HealthCheckStat, Target,
    base_parsers::{digits, param, target},
    types::{HostStat, HostStats, Param, Record},
};
use combine::{
    Parser, choice,
    error::ParseError,
    many, optional,
    parser::char::{newline, space, string},
    stream::Stream,
    token,
};

pub(crate) const MEMUSED_MAX: &str = "memused_max";
pub(crate) const MEMUSED: &str = "memused";
pub(crate) const LNET_MEMUSED: &str = "lnet_memused";
pub(crate) const HEALTH_CHECK: &str = "health_check";

pub(crate) const TOP_LEVEL_PARAMS: [&str; 4] = [MEMUSED, MEMUSED_MAX, LNET_MEMUSED, HEALTH_CHECK];

pub(crate) fn top_level_params() -> Vec<String> {
    TOP_LEVEL_PARAMS.iter().map(|x| (*x).to_string()).collect()
}

enum TopLevelStat {
    Memused(u64),
    MemusedMax(u64),
    LnetMemused(u64),
    HealthCheck(HealthCheckStat),
}

fn target_health<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("device").skip(space()),
        target().skip(space()),
        string("reported unhealthy"),
    )
        .map(|(_, target, _)| target)
}

fn targets_health<I>() -> impl Parser<I, Output = Vec<Target>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many(target_health().skip(newline()))
}

fn health_stats<I>() -> impl Parser<I, Output = HealthCheckStat>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (string("healthy").map(|_| HealthCheckStat {
            healthy: true,
            targets: vec![],
        })),
        (string("LBUG").map(|_| HealthCheckStat {
            healthy: false,
            targets: vec![],
        })),
        (string("NOT HEALTHY").map(|_| HealthCheckStat {
            healthy: false,
            targets: vec![],
        })),
        // Format: health_check=\ndevice...\ndevice...\n
        // targets_health() consumes all newlines, top_level_stat().skip(optional(newline())) handles the optional final one
        ((optional(newline()), targets_health()).map(|(_, targets)| HealthCheckStat {
            healthy: false,
            targets,
        })),
    ))
}

fn top_level_stat<I>() -> impl Parser<I, Output = (Param, TopLevelStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (param(MEMUSED), digits().map(TopLevelStat::Memused)),
        (param(MEMUSED_MAX), digits().map(TopLevelStat::MemusedMax)),
        (
            param(LNET_MEMUSED),
            (optional(token('-')), digits()).map(|(negative, x)| {
                if negative.is_some() {
                    // Counter can overflow and go negative.
                    // Cast stat to 0 when this happens
                    TopLevelStat::LnetMemused(0)
                } else {
                    TopLevelStat::LnetMemused(x)
                }
            }),
        ),
        (
            param(HEALTH_CHECK),
            health_stats().map(TopLevelStat::HealthCheck),
        ),
    ))
    .skip(optional(newline()))
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    top_level_stat()
        .map(|(param, v)| match v {
            TopLevelStat::Memused(value) => HostStats::Memused(HostStat { param, value }),
            TopLevelStat::MemusedMax(value) => HostStats::MemusedMax(HostStat { param, value }),
            TopLevelStat::LnetMemused(value) => HostStats::LNetMemUsed(HostStat { param, value }),
            TopLevelStat::HealthCheck(value) => HostStats::HealthCheck(HostStat { param, value }),
        })
        .map(Record::Host)
        .message("while parsing top_level_param")
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::types::{HostStat, HostStats, Param};

    #[test]
    fn test_params() {
        assert_eq!(
            top_level_params(),
            vec![
                "memused".to_string(),
                "memused_max".to_string(),
                "lnet_memused".to_string(),
                "health_check".to_string(),
            ]
        )
    }

    #[test]
    fn test_row() {
        let result = parse().parse("memused_max=77991501\n");

        assert_eq!(
            result,
            Ok((
                Record::Host(HostStats::MemusedMax(HostStat {
                    param: Param(MEMUSED_MAX.to_string()),
                    value: 77_991_501
                })),
                ""
            ))
        )
    }

    #[test]
    fn test_lnet_memused() {
        let result = parse().parse("lnet_memused=17448\n");

        assert_eq!(
            result,
            Ok((
                Record::Host(HostStats::LNetMemUsed(HostStat {
                    param: Param(LNET_MEMUSED.to_string()),
                    value: 17448
                })),
                ""
            ))
        )
    }

    #[test]
    fn test_negative_lnet_memused() {
        let result = parse().parse("lnet_memused=-1744897928\n");

        assert_eq!(
            result,
            Ok((
                Record::Host(HostStats::LNetMemUsed(HostStat {
                    param: Param(LNET_MEMUSED.to_string()),
                    value: 0
                })),
                ""
            ))
        )
    }

    #[test]
    fn test_health_check_healthy() {
        let output_str = "health_check=healthy\n";
        let result = parse().parse(output_str);

        assert_eq!(
            result,
            Ok((
                Record::Host(HostStats::HealthCheck(HostStat {
                    param: Param(HEALTH_CHECK.to_string()),
                    value: HealthCheckStat {
                        healthy: true,
                        targets: vec![]
                    }
                })),
                ""
            ))
        )
    }

    #[test]
    fn test_health_check_unhealthy() {
        let output_str = "health_check=NOT HEALTHY\n";
        let result = parse().parse(output_str);

        assert_eq!(
            result,
            Ok((
                Record::Host(HostStats::HealthCheck(HostStat {
                    param: Param(HEALTH_CHECK.to_string()),
                    value: HealthCheckStat {
                        healthy: false,
                        targets: vec![]
                    }
                })),
                ""
            ))
        )
    }

    #[test]
    fn test_lbug_health_check() {
        let result = parse().parse("health_check=LBUG\n");

        assert_eq!(
            result,
            Ok((
                Record::Host(HostStats::HealthCheck(HostStat {
                    param: Param(HEALTH_CHECK.to_string()),
                    value: HealthCheckStat {
                        healthy: false,
                        targets: vec![]
                    }
                })),
                ""
            ))
        )
    }

    #[test]
    fn test_health_check_unhealthy_with_devices() {
        // Test real-world output with two health_check records (single trailing newline)
        use combine::Parser;

        let output_str = r"health_check=NOT HEALTHY
health_check=
device lustre-OST0012 reported unhealthy
device lustre-OST0014 reported unhealthy
device lustre-OST0016 reported unhealthy
";
        let result = crate::parser::parse().parse(output_str);

        assert_eq!(
            result,
            Ok((
                vec![
                    Record::Host(HostStats::HealthCheck(HostStat {
                        param: Param(HEALTH_CHECK.to_string()),
                        value: HealthCheckStat {
                            healthy: false,
                            targets: vec![]
                        }
                    })),
                    Record::Host(HostStats::HealthCheck(HostStat {
                        param: Param(HEALTH_CHECK.to_string()),
                        value: HealthCheckStat {
                            healthy: false,
                            targets: vec![
                                Target("lustre-OST0012".to_string()),
                                Target("lustre-OST0014".to_string()),
                                Target("lustre-OST0016".to_string())
                            ]
                        }
                    })),
                ],
                ""
            ))
        )
    }

    #[test]
    fn test_health_check_unhealthy_with_single_device() {
        // Test real-world output with two health_check records (single trailing newline)
        use combine::Parser;

        let output_str = r"health_check=NOT HEALTHY
health_check=device lustre-OST0012 reported unhealthy
";
        let result = crate::parser::parse().parse(output_str);

        assert_eq!(
            result,
            Ok((
                vec![
                    Record::Host(HostStats::HealthCheck(HostStat {
                        param: Param(HEALTH_CHECK.to_string()),
                        value: HealthCheckStat {
                            healthy: false,
                            targets: vec![]
                        }
                    })),
                    Record::Host(HostStats::HealthCheck(HostStat {
                        param: Param(HEALTH_CHECK.to_string()),
                        value: HealthCheckStat {
                            healthy: false,
                            targets: vec![Target("lustre-OST0012".to_string())]
                        }
                    })),
                ],
                ""
            ))
        )
    }

    #[test]
    fn test_real_world_multi_param_with_unhealthy_devices() {
        // Test complete real-world lctl output from node3
        use combine::Parser;

        let output_str = r"memused=467705725
lnet_memused=20951852
health_check=NOT HEALTHY
health_check=
device lustre-OST0012 reported unhealthy
device lustre-OST0014 reported unhealthy
device lustre-OST0016 reported unhealthy
";
        let result = crate::parser::parse().parse(output_str);

        // Should parse all parameters successfully
        assert!(result.is_ok());
        let (records, remaining) = result.unwrap();

        // Should have consumed all input
        assert_eq!(remaining, "");

        // Should have 4 records
        insta::assert_debug_snapshot!(records, @r#"
        [
            Host(
                Memused(
                    HostStat {
                        param: Param(
                            "memused",
                        ),
                        value: 467705725,
                    },
                ),
            ),
            Host(
                LNetMemUsed(
                    HostStat {
                        param: Param(
                            "lnet_memused",
                        ),
                        value: 20951852,
                    },
                ),
            ),
            Host(
                HealthCheck(
                    HostStat {
                        param: Param(
                            "health_check",
                        ),
                        value: HealthCheckStat {
                            healthy: false,
                            targets: [],
                        },
                    },
                ),
            ),
            Host(
                HealthCheck(
                    HostStat {
                        param: Param(
                            "health_check",
                        ),
                        value: HealthCheckStat {
                            healthy: false,
                            targets: [
                                Target(
                                    "lustre-OST0012",
                                ),
                                Target(
                                    "lustre-OST0014",
                                ),
                                Target(
                                    "lustre-OST0016",
                                ),
                            ],
                        },
                    },
                ),
            ),
        ]
        "#);
    }
}
