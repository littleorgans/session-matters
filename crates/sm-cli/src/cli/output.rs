use sm_core::Session;

pub fn print_session_line(session: &Session) {
    println!(
        "{} {} {} {} {} {}",
        session.id,
        session.runtime,
        session.role,
        session.workspace,
        session.state,
        session.runtime_pid
    );
}

pub fn print_session_table(sessions: &[Session]) {
    println!("ID RUNTIME ROLE WORKSPACE STATE PID");
    for session in sessions {
        print_session_line(session);
    }
}
