#![allow(non_snake_case)]

use reqwest::Client;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use std::env;

// Number of worker tasks (threads) to spawn
const NUM_WORKERS: usize = 40 * 40;
// Default length of the words to generate if not provided in the command line arguments
const DEFAULT_WORD_LENGTH: usize = 2;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Read the word length from the comamnd line arguments or use the default value
    let wordLength = env::args()
        .nth(1)
        .map(|arg| arg.parse().unwrap_or(DEFAULT_WORD_LENGTH))
        .unwrap_or(DEFAULT_WORD_LENGTH);

    let client = Client::new();
    // Generate all combination of the specified word length
    let logins: Vec<String> = generateAllCombinations(wordLength);

    let (tx, rx) = mpsc::channel::<Option<String>>(100);
    let rx = Arc::new(Mutex::new(rx));
    let mut handles = Vec::new();

    // Open the file for writing valid logins
    let file = Arc::new(Mutex::new(OpenOptions::new()
        .create(true)
        .append(true)
        .open("valid_combinations.txt")?));

    let validLogins = Arc::new(Mutex::new(Vec::new()));

    // Spawn worker tasks
    for _ in 0..NUM_WORKERS {
        let client = client.clone();
        let rx = Arc::clone(&rx);
        let file = Arc::clone(&file);
        let validLogins = Arc::clone(&validLogins);

        handles.push(task::spawn(workerTask(client, rx, file, validLogins)));
    }

    // Send logins to worker tasks
    for login in logins {
        tx.send(Some(login)).await.expect("Failed to send login");
    }

    // Send termination signals to worker tasks
    for _ in 0..NUM_WORKERS {
        tx.send(None).await.expect("Failed to send termination signal");
    }

    // Await all worker tasks to complete
    for handle in handles {
        handle.await.expect("Task failed");
    }

    // Print the results
    let validLogins = validLogins.lock().await;
    println!();
    println!("Number of valid logins: {}", validLogins.len());
    for login in validLogins.iter() {
        println!("- {}", login)
    }

    Ok(())
}

// Worker task function
async fn workerTask(client: Client, rx: Arc<Mutex<mpsc::Receiver<Option<String>>>>, file: Arc<Mutex<std::fs::File>>, validLogins: Arc<Mutex<Vec<String>>>) {
    while let Some(login) = receiveLogin(&rx).await {
        if let Some(login) = login {
            handleLogin(&client, &login, &file, &validLogins).await;
        } else {
            break;
        }
    }
}

// Receive a login from the channel
async fn receiveLogin(rx: &Arc<Mutex<mpsc::Receiver<Option<String>>>>) -> Option<Option<String>> {
    let mut rx = rx.lock().await;
    rx.recv().await
}

// Handle processing of each login
async fn handleLogin(client: &Client, login: &str, file: &Arc<Mutex<std::fs::File>>, validLogins: &Arc<Mutex<Vec<String>>>) {
    if processLogin(client, login).await {
        let mut file = file.lock().await;
        writeln!(file, "{}", login).expect("Failed to write to file");

        let mut validLogins = validLogins.lock().await;
        validLogins.push(login.to_string());

        print!("+");
        io::stdout().flush().expect("Failed to flush stdout");
    } else {
        print!(".");
        io::stdout().flush().expect("Failed to flush stdout");
    }
}

// Process each login by sending a request to the Twitch API
async fn processLogin(client: &Client, login: &str) -> bool {
    let response = client.get(format!("https://passport.twitch.tv/usernames/{}", login))
        .send()
        .await
        .expect("Failed to send request");
    
    response.status().is_success() && response.status().as_u16() == 204
}

// Generate all combinations of the specified length
fn generateAllCombinations(length: usize) -> Vec<String> {
    let mut allCombinations = Vec::new();
    let alphabet = "abcdefghijklmnopqrstuvwxyz".chars().collect::<Vec<_>>();

    fn generate(current: &mut Vec<char>, length: usize, alphabet: &[char], allCombinations: &mut Vec<String>) {
        if current.len() == length {
            allCombinations.push(current.iter().collect());
            return;
        }
        for &c in alphabet {
            current.push(c);
            generate(current, length, alphabet, allCombinations);
            current.pop();
        }
    }

    generate(&mut Vec::new(), length, &alphabet, &mut allCombinations);
    allCombinations
}