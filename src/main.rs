use chain_addr::Discrimination;
use iced::{
    button, executor, scrollable, text_input, Align, Application, Button, Column, Command,
    Container, Element, HorizontalAlignment, Length, ProgressBar, Radio, Row, Scrollable, Settings,
    Space, Subscription, Text, TextInput,
};
use wallet_core as chain;

mod send_transaction;
mod wallet_state;

use wallet_state::AccountState;

const BLOCK0: &[u8] = include_bytes!("block0.bin");

pub fn main() {
    env_logger::init();

    let mut settings = Settings::default();

    settings.window.size = (1024, 768);
    settings.window.resizable = true;
    settings.window.decorations = true;

    settings.default_font = None;

    Tour::run(settings);
}

pub struct Tour {
    steps: Steps,
    scroll: scrollable::State,
    back_button: button::State,
    next_button: button::State,
    wallet: Wallet,
}

pub struct Wallet {
    wallet: Option<chain::Wallet>,
    settings: Option<chain::Settings>,
    proposal: chain::Proposal,
    vote: Option<Box<[u8]>>,
}

impl Wallet {
    pub fn new() -> Self {
        let id = "58993ca8d4721fb79b74413d4b8f7a4861c5b6426ac93efceda8f75c3e6f40eb".parse().unwrap();
        Self {
            wallet: None,
            settings: None,
            proposal: chain::Proposal::new(
                id,
                chain::PayloadType::Public,
                0,
                chain::Options::new_length(3).unwrap(),
            ),
            vote: None,
        }
    }

    pub fn recover(&mut self, mnemonics: &str) -> Result<(), chain::Error> {
        let mut wallet = chain::Wallet::recover(mnemonics, &[])?;
        let settings = wallet.retrieve_funds(BLOCK0)?;

        self.wallet = Some(wallet);
        self.settings = Some(settings);

        Ok(())
    }

    pub fn set_state(&mut self, value: chain::Value, counter: u32) {
        if let Some(wallet) = self.wallet.as_mut() {
            wallet.set_state(value, counter)
        }
    }

    pub fn make_choice(&mut self, choice: Choice) {
        self.vote = self
            .wallet
            .as_mut()
            .unwrap()
            .vote(
                self.settings.clone().unwrap(),
                &self.proposal,
                choice.into(),
            )
            .ok();
    }
}

impl Default for Wallet {
    fn default() -> Self {
        Self::new()
    }
}

impl Application for Tour {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Tour, Command<Message>) {
        (
            Tour {
                steps: Steps::new(),
                scroll: scrollable::State::new(),
                back_button: button::State::new(),
                next_button: button::State::new(),
                wallet: Wallet::new(),
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
            Message::StepMessage(step_msg) => self.steps.update(step_msg, &mut self.wallet),
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.steps.current() {
            Step::LoadState {
                loaded: None,
                progressed: _,
            } => {
                let address = chain_addr::AddressReadable::from_address(
                    "test",
                    &self
                        .wallet
                        .wallet
                        .as_ref()
                        .unwrap()
                        .account(Discrimination::Production),
                );
                dbg!(address.to_string());
                let url = format!(
                    "https://api.vit.iohk.io/api/v0/account/{}",
                    self.wallet.wallet.as_ref().unwrap().id()
                );

                wallet_state::query(url)
                    .map(|progress| StepMessage::State { progress })
                    .map(Message::StepMessage)
            }
            Step::WaitConfirmation {
                loaded: None,
                progressed: _,
            } => {
                let url = "https://api.vit.iohk.io/api/v0/message".to_owned();
                let body = self.wallet.vote.clone().unwrap_or_default();

                send_transaction::post(url, body)
                    .map(|progress| StepMessage::Transaction { progress })
                    .map(Message::StepMessage)
            }
            _ => Subscription::none(),
        }
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
            .max_width(800)
            .spacing(5)
            .padding(5)
            .push(steps.view().map(Message::StepMessage))
            .push(controls)
            .into();

        let scrollable =
            Scrollable::new(scroll).push(Container::new(content).width(Length::Fill).center_x());

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
                    error: None,
                },
                Step::LoadState {
                    loaded: None,
                    progressed: 0.0,
                },
                Step::Vote { choice: None },
                Step::WaitConfirmation {
                    loaded: None,
                    progressed: 0.0,
                },
                Step::End,
            ],
            current: 0,
        }
    }

    fn update(&mut self, msg: StepMessage, wallet: &mut Wallet) {
        self.steps[self.current].update(msg, wallet);
    }

    fn current(&self) -> &Step {
        self.steps.get(self.current).expect("cannot overflow")
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
        self.current + 1 < self.steps.len() && self.steps[self.current].can_continue()
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
        error: Option<chain::Error>,
    },
    LoadState {
        loaded: Option<Result<AccountState, String>>,
        progressed: f32,
    },
    Vote {
        choice: Option<Choice>,
    },
    WaitConfirmation {
        loaded: Option<Result<String, String>>,
        progressed: f32,
    },
    End,
}

#[derive(Debug, Clone)]
pub enum StepMessage {
    ChangeKey(String),
    State {
        progress: wallet_state::Progress,
    },
    Transaction {
        progress: send_transaction::Progress,
    },
    SelectVote(Choice),
}

impl<'a> Step {
    fn update(&mut self, msg: StepMessage, wallet: &mut Wallet) {
        match msg {
            StepMessage::ChangeKey(input) => {
                if let Step::EnterKey {
                    retrieved,
                    key,
                    state: _,
                    error,
                } = self
                {
                    *key = input;
                    *error = wallet.recover(&key).err();
                    *retrieved = wallet.wallet.is_some();
                }
            }
            StepMessage::State { progress } => {
                if let Step::LoadState { loaded, progressed } = self {
                    match progress {
                        wallet_state::Progress::Started => *progressed = 0.0,
                        wallet_state::Progress::Advanced(f) => *progressed = f,
                        wallet_state::Progress::Finished { account_state } => {
                            wallet.set_state(chain::Value(account_state.value), account_state.counter);
                            *loaded = Some(Ok(account_state));
                        }
                        wallet_state::Progress::Errored { status_code } => {
                            dbg!(status_code);
                            *loaded = Some(Err("Account not found".to_owned()));
                        }
                        wallet_state::Progress::Failure { error } => {
                            *loaded = Some(Err(format!("Error: {}", error)));
                        }
                    }
                }
            }
            StepMessage::Transaction { progress } => {
                if let Step::WaitConfirmation { loaded, progressed } = self {
                    match progress {
                        send_transaction::Progress::Started => *progressed = 0.0,
                        send_transaction::Progress::Advanced(f) => *progressed = f,
                        send_transaction::Progress::Finished { id } => {
                            *loaded = Some(Ok(id));
                        }
                        send_transaction::Progress::Errored { status_code } => {
                            dbg!(status_code);
                            *loaded = Some(Err("Cannot send vote".to_owned()));
                        }
                        send_transaction::Progress::Failure { error } => {
                            *loaded = Some(Err(format!("Error: {}", error)));
                        }
                    }
                }
            }
            StepMessage::SelectVote(new_choice) => {
                if let Step::Vote { choice, .. } = self {
                    *choice = Some(new_choice);
                    wallet.make_choice(new_choice);
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
            Step::LoadState {
                loaded,
                progressed: _,
            } => loaded.as_ref().map(|r| r.is_ok()).unwrap_or(false),
            Step::Vote { choice } => choice.is_some(),
            Step::WaitConfirmation {
                loaded,
                progressed: _,
            } => loaded.is_some(),
            Step::End => false,
        }
    }

    fn view(&mut self) -> Element<StepMessage> {
        match self {
            Step::Welcome => Self::welcome(),
            Step::EnterKey { key, state, error, .. } => Self::staking_wallet(key, state, error),
            Step::LoadState { loaded, progressed } => Self::view_get_state(*progressed, loaded),
            Step::Vote { choice } => Self::make_choice(choice),
            Step::WaitConfirmation { loaded, progressed } => {
                Self::view_send_vote(*progressed, loaded)
            }
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
",
            ))
            .push(Text::new(
                "To vote you only need your staking key. Either you have been using \
the account style wallet and it is straightforward your wallet's mnemonics. \
Or you have been using UTxO base wallet and you need to enter your stake private key.",
            ))
    }

    fn staking_wallet(key: &str, state: &'a mut text_input::State, error: &Option<chain::Error>) -> Column<'a, StepMessage> {
        let key_input = TextInput::new(state, "Inputs...", key, StepMessage::ChangeKey)
            .padding(10)
            .size(30);

        let error = if let Some(error) = error {
            Text::new(error.to_string())
        } else {
            Text::new("")
        };

        Self::container("Retrieve your stake key")
            .push(Text::new(
                "Use your account mnemonics or your StakeKey private key",
            ))
            .push(key_input)
            .push(error)
    }

    fn make_choice(choice: &Option<Choice>) -> Column<'a, StepMessage> {
        let question = Column::new()
            .padding(20)
            .spacing(10)
            .push(Text::new("Do you want to top up the reward pot of the ITN of 95M Ada?").size(24))
            .push(Choice::all().iter().cloned().fold(
                Column::new().padding(10).spacing(20),
                |choices, language| {
                    choices.push(Radio::new(
                        language,
                        language,
                        *choice,
                        StepMessage::SelectVote,
                    ))
                },
            ));

        Self::container("Cast your vote: The community needs you!").push(question)
    }

    fn view_get_state(
        current_progress: f32,
        data: &Option<Result<AccountState, String>>,
    ) -> Column<'a, StepMessage> {
        let progress_bar = ProgressBar::new(0.0..=100.0, current_progress);

        let control: Element<_> = if let Some(result) = data {
            match result {
                Ok(account_state) => Column::new()
                    .spacing(10)
                    .align_items(Align::Center)
                    .push(Text::new("Wallet synced finished!"))
                    .push(Text::new(format!(
                        "retrieved value {}",
                        account_state.value
                    )))
                    .push(Text::new(format!(
                        "retrieved counter {}",
                        account_state.counter
                    )))
                    .into(),
                Err(error) => Column::new()
                    .spacing(10)
                    .align_items(Align::Center)
                    .push(Text::new("Cannot sync the wallet!"))
                    .push(Text::new(error.to_owned()))
                    .into(),
            }
        } else {
            Text::new(format!("Downloading... {:.2}%", current_progress)).into()
        };
        let content = Column::new()
            .spacing(10)
            .padding(10)
            .align_items(Align::Center)
            .push(progress_bar)
            .push(control);

        Self::container("Retrieving wallet data").push(content)
    }

    fn view_send_vote(
        current_progress: f32,
        data: &Option<Result<String, String>>,
    ) -> Column<'a, StepMessage> {
        let progress_bar = ProgressBar::new(0.0..=100.0, current_progress);

        let control: Element<_> = if let Some(result) = data {
            match result {
                Ok(state) => Column::new()
                    .spacing(10)
                    .align_items(Align::Center)
                    .push(Text::new("Vote sent successfully!"))
                    .push(Text::new(format!(
                        "The transaction id '{}' can be used to confirm the vote transaction ont the explorer",
                        state
                    )))
                    .into(),
                Err(error) => Column::new()
                    .spacing(10)
                    .align_items(Align::Center)
                    .push(Text::new("Cannot send the transaction!"))
                    .push(Text::new(error.to_owned()))
                    .into(),
            }
        } else {
            Text::new(format!("Sending vote... {:.2}%", current_progress)).into()
        };
        let content = Column::new()
            .spacing(10)
            .padding(10)
            .align_items(Align::Center)
            .push(progress_bar)
            .push(control);

        Self::container("Sending vote to the blockchain").push(content)
    }

    fn end() -> Column<'a, StepMessage> {
        Self::container("Thank you so much for your contribution!")
            .push(Text::new(
                "It has been such a long journey. Whatever the choice you made it \
                The Jörmungandr Team thanks you for your contribution and support.",
            ))
            .push(Text::new(
                "We will make announcement shortly after the results \
            so stay tune.",
            ))
    }
}

fn button<'a, Message>(state: &'a mut button::State, label: &str) -> Button<'a, Message> {
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
        [Choice::Blank, Choice::Yes, Choice::No]
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
