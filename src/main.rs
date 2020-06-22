use iced::{
    button, executor, scrollable, slider, text_input, Application, Button,
    Checkbox, Color, Column, Command, Container, Element, HorizontalAlignment,
    Length, Radio, Row, Scrollable, Settings, Slider, Space, Text,
    TextInput,
};
use wallet_core as chain;

pub fn main() {
    env_logger::init();

    Tour::run(Settings::default())
}

pub struct Tour {
    steps: Steps,
    scroll: scrollable::State,
    back_button: button::State,
    next_button: button::State,
    wallet: Option<chain::Wallet>,
}

impl Application for Tour {
    type Executor = executor::Null;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Tour, Command<Message>) {
        (
            Tour {
                steps: Steps::new(),
                scroll: scrollable::State::new(),
                back_button: button::State::new(),
                next_button: button::State::new(),
                wallet: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        format!("{} - Jorvot", self.steps.title())
    }

    fn update(&mut self, event: Message) -> Command<Message> {
        match event {
            Message::BackPressed => {
                self.steps.go_back();
            }
            Message::NextPressed => {
                self.steps.advance();
            }
            Message::StepMessage(step_msg) => {
                self.steps.update(step_msg, &mut self.wallet)
            }
        }

        Command::none()
    }

    fn view(&mut self) -> Element<Message> {
        let Tour {
            steps,
            scroll,
            back_button,
            next_button,
            ..
        } = self;

        let mut controls = Row::new();

        if steps.has_previous() {
            controls = controls.push(
                button(back_button, "Back")
                    .on_press(Message::BackPressed)
                    .style(style::Button::Secondary),
            );
        }

        controls = controls.push(Space::with_width(Length::Fill));

        if steps.can_continue() {
            controls = controls.push(
                button(next_button, "Next")
                    .on_press(Message::NextPressed)
                    .style(style::Button::Primary),
            );
        }

        let content: Element<_> = Column::new()
            .max_width(540)
            .spacing(20)
            .padding(20)
            .push(steps.view().map(Message::StepMessage))
            .push(controls)
            .into();

        let scrollable = Scrollable::new(scroll)
            .push(Container::new(content).width(Length::Fill).center_x());

        Container::new(scrollable)
            .height(Length::Fill)
            .center_y()
            .into()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    BackPressed,
    NextPressed,
    StepMessage(StepMessage),
}

struct Steps {
    steps: Vec<Step>,
    current: usize,
}

impl Steps {
    fn new() -> Steps {
        Steps {
            steps: vec![
                Step::Welcome,
                Step::EnterKey {
                    key: String::new(),
                    retrieved: false,
                    state: text_input::State::new(),
                },
                Step::LoadState {
                    loaded: false,
                },
                Step::Vote {
                    choice: None,
                },
                Step::WaitConfirmation { confirmed: false },
                Step::End,
            ],
            current: 0,
        }
    }

    fn update(&mut self, msg: StepMessage, wallet: &mut Option<chain::Wallet>) {
        self.steps[self.current].update(msg, wallet);
    }

    fn view(&mut self) -> Element<StepMessage> {
        self.steps[self.current].view()
    }

    fn advance(&mut self) {
        if self.can_continue() {
            self.current += 1;
        }
    }

    fn go_back(&mut self) {
        if self.has_previous() {
            self.current -= 1;
        }
    }

    fn has_previous(&self) -> bool {
        self.current > 0
    }

    fn can_continue(&self) -> bool {
        self.current + 1 < self.steps.len()
            && self.steps[self.current].can_continue()
    }

    fn title(&self) -> &str {
        self.steps[self.current].title()
    }
}

#[allow(clippy::large_enum_variant)]
enum Step {
    Welcome,
    EnterKey {
        key: String,
        retrieved: bool,
        state: text_input::State,
    },
    LoadState {
        loaded: bool,
    },
    Vote {
        choice: Option<Choice>
    },
    WaitConfirmation { confirmed: bool },
    End,
}

#[derive(Debug, Clone)]
pub enum StepMessage {
    ChangeKey(String),
    State { value: chain::Value, counter: u32 },
    SelectVote(Choice),
}

impl<'a> Step {
    fn update(&mut self, msg: StepMessage, wallet: &mut Option<chain::Wallet>) {
        match msg {
            StepMessage::ChangeKey(input) => {
                if let Step::EnterKey { retrieved, key, state: _ } = self {
                    *key = input;
                    *wallet = chain::Wallet::recover(&key, &[]).ok();
                    *retrieved = wallet.is_some();
                }
            }
            StepMessage::State { value, counter } => {
                if let Some(wallet) = wallet {
                    wallet.set_state(value, counter);

                    if let Step::LoadState { loaded } = self {
                        *loaded = true;
                    }
                }
            }
            StepMessage::SelectVote(new_choice) => {
                if let Step::Vote { choice, .. } = self {
                    *choice = Some(new_choice);
                }
            }
        };
    }

    fn title(&self) -> &str {
        match self {
            Step::Welcome => "Welcome",
            Step::EnterKey { .. } => "Register",
            Step::LoadState { .. } => "Registering",
            Step::Vote { .. } => "Vote",
            Step::WaitConfirmation { .. } => "Confirming",
            Step::End => "Thank you for your contribution",
        }
    }

    fn can_continue(&self) -> bool {
        match self {
            Step::Welcome => true,
            Step::EnterKey { retrieved, .. } => *retrieved,
            Step::LoadState { loaded } => *loaded,
            Step::Vote { choice } => choice.is_some(),
            Step::WaitConfirmation { confirmed } => *confirmed,
            Step::End => false,
        }
    }

    fn view(&mut self) -> Element<StepMessage> {
        match self {
            Step::Welcome => Self::welcome(),
            Step::EnterKey { key, state, .. } => Self::staking_wallet(key, state),
            Step::LoadState { loaded: false } => unimplemented!(),
            Step::LoadState { loaded: true } => unimplemented!(),
            Step::Vote { choice } => unimplemented!(),
            Step::WaitConfirmation { confirmed } => unimplemented!(),
            Step::End => Self::end(),
        }
        .into()
    }

    fn container(title: &str) -> Column<'a, StepMessage> {
        Column::new().spacing(20).push(Text::new(title).size(50))
    }

    fn welcome() -> Column<'a, StepMessage> {
        Self::container("Welcome!")
            .push(Text::new(
"The Incentivised TestNet has been running for more than 6 months. \
Seeing how the community is dedicated to the Jörmungandr node's progress \
We thought we would give you an opportunity to vote to decide its fate.\
"
            ))
            .push(Text::new(
"To vote you only need your staking key. Either you have been using \
the account style wallet and it is straightforward your wallet's mnemonics. \
Or you have been using UTxO base wallet and you need to enter your stake private key."
            ))
    }

    fn staking_wallet(key: &str, state: &'a mut text_input::State) -> Column<'a, StepMessage> {
        let key_input = TextInput::new(
            state,
            "Inputs...",
            key,
            StepMessage::ChangeKey
        ).padding(10).size(30);


        Self::container("Retrieve your stake key")
            .push(Text::new("Use your account mnemonics or your StakeKey private key"))
            .push(key_input)
    }

    fn end() -> Column<'a, StepMessage> {
        Self::container("Thank you so much for your contribution!")
            .push(Text::new(
                "It has been such a long journey. Whatever the choice you made it \
                The Jörmungandr Team thanks you for your contribution and support.",
            ))
            .push(Text::new("We will make announcement shortly after the results \
            so stay tune."))
    }
}

fn button<'a, Message>(
    state: &'a mut button::State,
    label: &str,
) -> Button<'a, Message> {
    Button::new(
        state,
        Text::new(label).horizontal_alignment(HorizontalAlignment::Center),
    )
    .padding(12)
    .min_width(100)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Blank,
    Yes,
    No,
}

impl Choice {
    fn all() -> [Choice; 3] {
        [
            Choice::Blank,
            Choice::Yes,
            Choice::No,
        ]
    }
}

impl From<Choice> for chain::Choice {
    fn from(choice: Choice) -> Self {
        chain::Choice::new(match choice {
            Choice::Blank => 0,
            Choice::Yes => 1,
            Choice::No => 2,
        })
    }
}

impl From<Choice> for String {
    fn from(choice: Choice) -> String {
        String::from(match choice {
            Choice::Blank => "Blank",
            Choice::Yes => "Yes",
            Choice::No => "No",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Row,
    Column,
}

mod style {
    use iced::{button, Background, Color, Vector};

    pub enum Button {
        Primary,
        Secondary,
    }

    impl button::StyleSheet for Button {
        fn active(&self) -> button::Style {
            button::Style {
                background: Some(Background::Color(match self {
                    Button::Primary => Color::from_rgb(0.11, 0.42, 0.87),
                    Button::Secondary => Color::from_rgb(0.5, 0.5, 0.5),
                })),
                border_radius: 12,
                shadow_offset: Vector::new(1.0, 1.0),
                text_color: Color::from_rgb8(0xEE, 0xEE, 0xEE),
                ..button::Style::default()
            }
        }

        fn hovered(&self) -> button::Style {
            button::Style {
                text_color: Color::WHITE,
                shadow_offset: Vector::new(1.0, 2.0),
                ..self.active()
            }
        }
    }
}