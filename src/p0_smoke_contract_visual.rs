use rshell_ui::SmokeReport;

pub(crate) fn visual_matrix_is_exact(report: Option<&SmokeReport>) -> bool {
    report.is_some_and(|report| {
        let visual = &report.counters.visual;
        let required = rshell_ui::visual_matrix();
        visual.len() == required.len()
            && report.requested_png_paths.len() == visual.len()
            && report.png_paths.len() == visual.len()
            && required.iter().all(|(width, height, state, mode)| {
                visual
                    .values()
                    .filter(|evidence| {
                        evidence.state == *state
                            && evidence.layout == *mode
                            && evidence.facts.requested_width == *width
                            && evidence.facts.requested_height == *height
                            && evidence.contract_passes()
                    })
                    .count()
                    == 1
            })
    })
}
