//! Integration tests kiểm tra luồng hoạt động đầu-cuối của `vntype-engine`.

use std::cell::RefCell;
use std::rc::Rc;

use vntype_engine::{
    Action, Config, FieldKind, ImeSession, Key, KeyEvent, MemoryConfigStore, OutputMode, Ports,
    SharedConfigStore, StaticAppContext, SurroundingText, shared_dictionary,
};

struct BufSurrounding(Rc<RefCell<String>>);

impl SurroundingText for BufSurrounding {
    fn before_cursor(&self, n: usize) -> Option<String> {
        let s = self.0.borrow();
        let skip = s.chars().count().saturating_sub(n);
        Some(s.chars().skip(skip).collect())
    }
}

#[test]
fn end_to_end_session_replace_and_macro_flow() {
    let mut cfg = Config::default();
    cfg.macros.insert("vn".into(), "Việt Nam".into());
    let store = SharedConfigStore(Rc::new(MemoryConfigStore::new(cfg)));
    let screen = Rc::new(RefCell::new(String::new()));

    let ports = Ports {
        config: Box::new(store.clone()),
        app: Box::new(StaticAppContext::new("chrome")),
        surrounding: Box::new(BufSurrounding(screen.clone())),
        dictionary: Box::new(shared_dictionary()),
    };

    let mut session = ImeSession::new(ports, OutputMode::Replace);
    assert!(session.is_enabled());

    for ch in "Tooi yeue vn".chars() {
        let act = session.process_key(&KeyEvent::ch(ch));
        match act {
            Action::PassThrough => screen.borrow_mut().push(ch),
            Action::Replace { delete_before, insert, pass_key } => {
                let mut buf = screen.borrow_mut();
                let keep = buf.chars().count().saturating_sub(delete_before);
                *buf = buf.chars().take(keep).collect::<String>() + &insert;
                if pass_key {
                    buf.push(ch);
                }
            }
            _ => unreachable!(),
        }
    }

    let end_act = session.process_key(&KeyEvent::new(Key::Enter));
    if let Action::Replace { delete_before, insert, .. } = end_act {
        let mut buf = screen.borrow_mut();
        let keep = buf.chars().count().saturating_sub(delete_before);
        *buf = buf.chars().take(keep).collect::<String>() + &insert;
    }
    assert_eq!(&*screen.borrow(), "Tôi yêu Việt Nam");

    session.set_field_kind(FieldKind::Password);
    assert_eq!(session.process_key(&KeyEvent::ch('x')), Action::PassThrough);
}
