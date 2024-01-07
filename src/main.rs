// Import necessary libraries
use futures_util::stream::StreamExt;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::str::FromStr;

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
struct WebSocketMessage {
    e: String,           // Event type
    E: u64,              // Event time
    s: String,           // Symbol
    U: u64,              // First update ID in event
    u: u64,              // Final update ID in event
    b: Vec<[String; 2]>, // Bids to be updated
    a: Vec<[String; 2]>, // Asks to be updated
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Order {
    price: Decimal,
    quantity: Decimal,
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        self.price.partial_cmp(&other.price).unwrap()
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct OrderBook {
    bids: BinaryHeap<Order>,
    asks: BinaryHeap<Reverse<Order>>, // Min-heap for asks
}

impl OrderBook {
    fn new() -> Self {
        OrderBook {
            bids: BinaryHeap::new(),
            asks: BinaryHeap::new(),
        }
    }

    // Function to add orders to the order book
    fn add_order(&mut self, order: Order, order_type: &str) {
        match order_type {
            "bid" => self.bids.push(order),
            "ask" => self.asks.push(Reverse(order)),
            _ => panic!("Unknown order type"),
        }
    }
}

#[tokio::main]
async fn main() {
    // Define the WebSocket URL for the Binance order book stream
    let url = Url::parse("wss://stream.binance.com:9443/ws/btcusdt@depth").unwrap();

    // Connect to the WebSocket
    let (mut socket, response) = connect_async(url).await.expect("Failed to connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");
    for (ref header, _value) in response.headers() {
        println!("{}", header);
    }

    // Listen for messages
    while let Some(message) = socket.next().await {
        let message = message.unwrap();

        // Handle different types of messages here
        match message {
            Message::Text(text) => handle_message(text),
            Message::Binary(bin) => println!("Received binary data: {:?}", bin),
            _ => (),
        }
    }
}

fn handle_message(text: String) {
    // Parse the message text and update order book data

    let mut order_book = OrderBook::new();

    let message: WebSocketMessage = serde_json::from_str(&text).unwrap();

    for bid in message.b {
        let price = Decimal::from_str(&bid[0]).unwrap();
        let quantity = Decimal::from_str(&bid[1]).unwrap();

        // Add or update the bid in your order book
        order_book.add_order(Order { price, quantity }, "bid");
    }

    for ask in message.a {
        let price = Decimal::from_str(&ask[0]).unwrap();
        let quantity = Decimal::from_str(&ask[1]).unwrap();

        // Add or update the ask in your order book
        order_book.add_order(Order { price, quantity }, "ask");
    }

    // Calculate the weighted average price for the first 10 bids and asks
    let depth = Decimal::from_str("1").unwrap(); // Example depth

    let bid_avg_price =
        calculate_weighted_average_price_bids(&order_book.bids, depth).unwrap_or(Decimal::ZERO);

    let ask_avg_price =
        calculate_weighted_average_price_asks(&order_book.asks, depth).unwrap_or(Decimal::ZERO);

    let spread = ask_avg_price - bid_avg_price;

    // Format and print the values
    // Convert Decimal to a primitive float for easy formatting (consider precision needs)
    println!(
        "Best Bid: {:.2}, Best Ask: {:.2}, Spread: {:.2}",
        bid_avg_price.to_f64().unwrap_or(0.0),
        ask_avg_price.to_f64().unwrap_or(0.0),
        spread.to_f64().unwrap_or(0.0)
    );
}

// Calculate Weighted Average Price for Bids
fn calculate_weighted_average_price_bids(
    bids: &BinaryHeap<Order>,
    depth: Decimal,
) -> Option<Decimal> {
    let mut weighted_sum = Decimal::ZERO;
    let mut total_quantity = Decimal::ZERO;

    for order in bids.iter() {
        if total_quantity + order.quantity >= depth {
            let remaining_quantity = depth - total_quantity;
            weighted_sum += order.price * remaining_quantity;
            total_quantity += remaining_quantity;
            break;
        } else {
            weighted_sum += order.price * order.quantity;
            total_quantity += order.quantity;
        }
    }

    if total_quantity.is_zero() {
        None
    } else {
        Some(weighted_sum / total_quantity)
    }
}

// Calculate Weighted Average Price for Asks
fn calculate_weighted_average_price_asks(
    asks: &BinaryHeap<Reverse<Order>>,
    depth: Decimal,
) -> Option<Decimal> {
    let mut weighted_sum = Decimal::ZERO;
    let mut total_quantity = Decimal::ZERO;

    for reverse_order in asks.iter() {
        let order = &reverse_order.0; // Extract the Order from Reverse<Order>
        if total_quantity + order.quantity >= depth {
            let remaining_quantity = depth - total_quantity;
            weighted_sum += order.price * remaining_quantity;
            total_quantity += remaining_quantity;
            break;
        } else {
            weighted_sum += order.price * order.quantity;
            total_quantity += order.quantity;
        }
    }

    if total_quantity.is_zero() {
        None
    } else {
        Some(weighted_sum / total_quantity)
    }
}
