//! Script do pong — porte literal de `handlers/pong-script.ts`. Separado do template HTML
//! ([`super::minigame_pong`]) só para manter os dois arquivos abaixo do teto de 300 linhas do
//! AGENTS.md. `__WIN_SCORE__` é substituído em runtime (mesma interpolação `${WIN_SCORE}` do TS).

pub const WIN_SCORE: u32 = 7;

// `r##"..."##` (2 cercas, não 1): o script tem cores como `"#8b5cf6"` — a sequência `"#` fecharia um
// `r#"..."#` no meio do conteúdo.
pub const PONG_SCRIPT: &str = r##"
    const canvas = document.getElementById("pong");
    const ctx = canvas.getContext("2d");

    const player = { x: 10, y: 150, w: 10, h: 80, score: 0, color: "#8b5cf6" };
    const mika = { x: 580, y: 150, w: 10, h: 80, score: 0, color: "#ec4899" };
    const ball = { x: 300, y: 200, r: 8, speed: 5, dx: 5, dy: 5, color: "#10b981" };

    const WIN_SCORE = __WIN_SCORE__;
    let gameActive = true;

    let upPressed = false;
    let downPressed = false;

    window.addEventListener("keydown", (e) => {
      if (e.key === "ArrowUp" || e.key === "w" || e.key === "W") upPressed = true;
      if (e.key === "ArrowDown" || e.key === "s" || e.key === "S") downPressed = true;
    });

    window.addEventListener("keyup", (e) => {
      if (e.key === "ArrowUp" || e.key === "w" || e.key === "W") upPressed = false;
      if (e.key === "ArrowDown" || e.key === "s" || e.key === "S") downPressed = false;
    });

    function drawRect(x, y, w, h, color) {
      ctx.fillStyle = color;
      ctx.fillRect(x, y, w, h);
    }

    function drawCircle(x, y, r, color) {
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2, false);
      ctx.closePath();
      ctx.fill();
    }

    function drawNet() {
      for (let i = 0; i <= canvas.height; i += 15) {
        drawRect(canvas.width / 2 - 1, i, 2, 10, "#1f2937");
      }
    }

    function resetBall() {
      ball.x = canvas.width / 2;
      ball.y = canvas.height / 2;
      ball.speed = 5;
      ball.dx = -Math.sign(ball.dx || 1) * ball.speed;
      ball.dy = 0;
    }

    function update() {
      if (!gameActive) return;

      // Player movement
      if (upPressed && player.y > 0) player.y -= 7;
      if (downPressed && player.y < canvas.height - player.h) player.y += 7;

      // Simple Mika AI (follows ball with delay)
      const mikaCenter = mika.y + mika.h / 2;
      if (mikaCenter < ball.y - 15 && mika.y < canvas.height - mika.h) {
        mika.y += 4.5;
      } else if (mikaCenter > ball.y + 15 && mika.y > 0) {
        mika.y -= 4.5;
      }

      // Ball movement
      ball.x += ball.dx;
      ball.y += ball.dy;

      // Ball boundary collision (top/bottom)
      if (ball.y - ball.r < 0 || ball.y + ball.r > canvas.height) {
        ball.dy = -ball.dy;
      }

      // Detect collision
      const paddle = (ball.x < canvas.width / 2) ? player : mika;

      let pTop = paddle.y;
      let pBottom = paddle.y + paddle.h;
      let pLeft = paddle.x;
      let pRight = paddle.x + paddle.w;

      let bTop = ball.y - ball.r;
      let bBottom = ball.y + ball.r;
      let bLeft = ball.x - ball.r;
      let bRight = ball.x + ball.r;

      if (bRight > pLeft && bLeft < pRight && bBottom > pTop && bTop < pBottom) {
        // Collided, bounce back and adjust angle
        let collidePoint = (ball.y - (paddle.y + paddle.h / 2));
        collidePoint = collidePoint / (paddle.h / 2);
        let angleRad = (Math.PI / 4) * collidePoint;
        let direction = (ball.x < canvas.width / 2) ? 1 : -1;
        ball.speed += 0.5;
        ball.dx = direction * ball.speed * Math.cos(angleRad);
        ball.dy = ball.speed * Math.sin(angleRad);

        // Reposition ball outside the paddle bounding box in the same frame
        // so the overlap cannot re-trigger this collision on the next frame.
        ball.x = (direction === 1) ? pRight + ball.r : pLeft - ball.r;
      }

      // Score update
      if (ball.x - ball.r < 0) {
        mika.score++;
        document.getElementById("mika-score").innerText = mika.score;
        if (mika.score >= WIN_SCORE) {
          endMatch("Mika venceu! 😈", "#ef4444");
          return;
        }
        resetBall();
      } else if (ball.x + ball.r > canvas.width) {
        player.score++;
        document.getElementById("player-score").innerText = player.score;
        if (player.score >= WIN_SCORE) {
          endMatch("Você venceu! 🎉", "#10b981");
          return;
        }
        resetBall();
      }
    }

    function endMatch(message, color) {
      gameActive = false;
      const status = document.getElementById("status");
      status.innerText = message;
      status.style.color = color;
    }

    function render() {
      // Clear canvas
      drawRect(0, 0, canvas.width, canvas.height, "#09090b");
      drawNet();
      drawRect(player.x, player.y, player.w, player.h, player.color);
      drawRect(mika.x, mika.y, mika.w, mika.h, mika.color);
      drawCircle(ball.x, ball.y, ball.r, ball.color);
    }

    function loop() {
      update();
      render();
      if (gameActive) {
        requestAnimationFrame(loop);
      }
    }

    loop();
"##;
