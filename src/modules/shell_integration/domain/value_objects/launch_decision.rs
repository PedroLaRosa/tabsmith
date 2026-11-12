/// Whether tabsmith launches (SPEC §5.2) and the reason line `_ should-launch` prints (`✅ …` / `❌ …`).
#[derive(Debug)]
pub(crate) struct LaunchDecision {
    launches: bool,
    reason: String,
}

impl LaunchDecision {
    pub(crate) fn new(launches: bool, reason: String) -> Self {
        LaunchDecision { launches, reason }
    }

    pub(crate) fn launches(&self) -> bool {
        self.launches
    }

    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_tell_whether_to_launch_and_the_reason_line_to_print() {
        let decision = LaunchDecision::new(false, "❌ In CI".to_string());

        let told = (decision.launches(), decision.reason());

        assert_eq!(told, (false, "❌ In CI"));
    }
}
