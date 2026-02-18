use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, time::Duration};
use std::process::Command;

enum InputMode {
    Normal,
    EditingSource,
    EditingRegex,
}

struct App {
    source_text: String,
    regex_input: String,
    output_text: String,
    input_mode: InputMode,
    status_message: String,
}

impl Default for App {
    fn default() -> App {
        App {
            source_text: "Hola Gabriel! Proba escribir algo aca.".to_string(),
            regex_input: String::new(),
            output_text: String::new(),
            input_mode: InputMode::Normal,
            status_message: "Listo. 's' editar texto, 'r' regex, 'TAB' IA".to_string(),
        }
    }
}

impl App {
    fn apply_regex(&mut self) {
        if self.regex_input.is_empty() {
            self.output_text = self.source_text.clone();
            return;
        }

        let re = match regex::Regex::new(&self.regex_input) {
            Ok(r) => r,
            Err(e) => {
                self.output_text = format!("Regex Error: {}", e);
                return;
            }
        };

        let mut out = String::new();
        for line in self.source_text.lines() {
            if re.is_match(line) {
                out.push_str(line);
                out.push('\n');
            }
        }
        self.output_text = out;
    }

    fn suggest_ai(&mut self) {
        self.status_message = "Consultando a Gemini IA...".to_string();
        
        let prompt = format!(
            "Give me ONLY the regex pattern (no text, no backticks) to match this: '{}' in the text: '{}'.",
            self.regex_input, self.source_text
        );

        // Cambiamos a gemini -p para modo no interactivo
        let output = Command::new("gemini")
            .arg("-p")
            .arg(prompt)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let suggestion = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !suggestion.is_empty() {
                    // Limpiamos posibles formatos de markdown que a veces devuelve la IA
                    let clean = suggestion
                        .replace("```regex", "")
                        .replace("```", "")
                        .replace("`", "")
                        .trim()
                        .to_string();
                    self.regex_input = clean;
                    self.status_message = "Sugerencia de Gemini aplicada!".to_string();
                    self.apply_regex();
                } else {
                    self.status_message = "Gemini no devolvió una respuesta clara.".to_string();
                }
            }
            _ => {
                self.status_message = "Error: no se pudo ejecutar 'gemini -p'".to_string();
            }
        }
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::default();
    app.apply_regex(); 
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app)).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // CORRECCION A: Evitar duplicación filtrando solo eventos de presión
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('s') => {
                            app.input_mode = InputMode::EditingSource;
                            app.source_text.clear(); // Limpiamos para que escribas de cero
                        }
                        KeyCode::Char('r') => {
                            app.input_mode = InputMode::EditingRegex;
                            app.regex_input.clear(); // Limpiamos para la nueva regex
                        }
                        KeyCode::Tab => {
                            app.suggest_ai();
                        }
                        _ => {}
                    },
                    InputMode::EditingSource => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::Normal,
                        KeyCode::Char(c) => {
                            app.source_text.push(c);
                        }
                        KeyCode::Backspace => {
                            app.source_text.pop();
                        }
                        KeyCode::Enter => {
                            app.source_text.push('\n');
                        }
                        _ => {
                            // Aplicar regex cada vez que cambia el texto
                            app.apply_regex();
                        }
                    },
                    InputMode::EditingRegex => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::Normal,
                        KeyCode::Char(c) => {
                            app.regex_input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.regex_input.pop();
                        }
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                }
                // Actualizar preview después de cualquier cambio
                app.apply_regex();
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), 
                Constraint::Min(5),    
                Constraint::Length(3), 
                Constraint::Min(5),    
                Constraint::Length(3), 
            ]
            .as_ref(),
        )
        .split(area);

    let title_text = format!(" REGEX WYSIWYG - MODO: {:?} ", match app.input_mode {
        InputMode::Normal => "EXPLORAR",
        InputMode::EditingSource => "EDITANDO FUENTE",
        InputMode::EditingRegex => "EDITANDO REGEX",
    });

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let source_style = if matches!(app.input_mode, InputMode::EditingSource) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let source = Paragraph::new(app.source_text.as_str())
        .style(source_style)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" [Source Text] (Press 's' para limpiar y editar) "));
    f.render_widget(source, chunks[1]);

    let regex_style = if matches!(app.input_mode, InputMode::EditingRegex) {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default()
    };
    let regex_input = Paragraph::new(app.regex_input.as_str())
        .style(regex_style)
        .block(Block::default().borders(Borders::ALL).title(" [Regex / IA Prompt] (Press 'r' para editar) "));
    f.render_widget(regex_input, chunks[2]);

    let output = Paragraph::new(app.output_text.as_str())
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title(" [Output Preview] "));
    f.render_widget(output, chunks[3]);

    let help_text = match app.input_mode {
        InputMode::Normal => format!("{} | q: Salir", app.status_message),
        _ => "Esc: Confirmar | Backspace: Borrar".to_string(),
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[4]);
}
