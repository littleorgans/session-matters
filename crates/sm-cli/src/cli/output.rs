use sm_core::{Mail, Session};

pub fn print_session_line(session: &Session) {
    println!(
        "{} {} {} {} {} {} {} {}",
        session.id,
        session.runtime,
        session.role,
        session.namespace,
        session.dir.display(),
        session.state,
        session.runtime_pid,
        session.tmux_pane.as_deref().unwrap_or("-")
    );
}

pub fn print_session_table(sessions: &[Session]) {
    println!("ID RUNTIME ROLE NAMESPACE DIR STATE PID TMUX");
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
