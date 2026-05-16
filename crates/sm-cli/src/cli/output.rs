use sm_core::{Mail, Session};

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

pub fn print_mail(mail: &[Mail]) {
    for item in mail {
        println!(
            "{} {} {} {} {}",
            item.id,
            item.sender_id,
            item.recipient_id,
            item.status(),
            item.content
        );
    }
}
