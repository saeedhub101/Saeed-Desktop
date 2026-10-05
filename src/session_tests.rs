#[cfg(test)]
mod tests {
    use super::Session;

    #[test]
    fn preserves_shared_conversation_order() {
        let mut session = Session::new();
        session.push("user", "open the salary file");
        session.push("assistant", "I will open it");
        session.push("user", "calculate Ahmed's total");

        assert_eq!(session.messages().len(), 3);
        assert_eq!(session.messages()[2].content, "calculate Ahmed's total");
    }
}
