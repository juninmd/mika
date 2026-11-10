//! CSS compartilhado do "shell" (body) dos minigames — porte literal de `handlers/game-shell-css.ts`.

pub const GAME_SHELL_BODY_CSS: &str = r#"body {
      margin: 0;
      background: #0d0d15;
      color: #fff;
      font-family: 'Courier New', monospace;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      height: 100vh;
      overflow: hidden;
    }"#;
