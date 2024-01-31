function redirectToMovies() {
	window.location.href = '/movies';
}

function redirectToLoginPage() {
	window.location.href = '/login';
}

function redirectToAddMoviePage() {
	window.location.href = '/add-movie';
}

function redirectToDeleteMoviePage() {
	window.location.href = '/delete-movie';
}

function getUsernameFromCookie () {
	const usernameCookie = document.cookie.split('; ').find(cookie => cookie.startsWith('username='));
	return usernameCookie ? usernameCookie.split('=')[1] : null;
};

function displayUsernameAndSetButtons() {
	const usernameDisplay = document.getElementById('username-display');
	const username = getUsernameFromCookie();
	if (usernameDisplay && username) {
		usernameDisplay.textContent = 'Welcome, ' + username + '!';
		usernameDisplay.style.display = 'inline-block';

		const logoutButton = document.getElementById('logout-button');
		if (logoutButton) {
			logoutButton.style.display = 'inline-block';
		}
		const moviesButton = document.getElementById('movies-button');
		if (moviesButton) {
			moviesButton.style.display = 'inline-block';
		}
		const loginButton = document.getElementById('login-button');
		if (loginButton) {
			loginButton.style.display = 'none';
		}
	} else {
		usernameDisplay.style.display = 'none';
		const logoutButton = document.getElementById('logout-button');
		if (logoutButton) {
			logoutButton.style.display = 'none';
		}
		const moviesButton = document.getElementById('movies-button');
		if (moviesButton) {
			moviesButton.style.display = 'none';
		}
		const loginButton = document.getElementById('login-button');
		if (loginButton) {
			loginButton.style.display = 'inline-block';
		}
		const addMoviesButton = document.getElementById('add-movies-button');
		if (addMoviesButton) {
			addMoviesButton.style.display = 'none';
		}

	}
}

function cleanInterface() {
	const messageContainer = document.getElementById('message-container');
	messageContainer.textContent = '';
	const errorContainer = document.getElementById('error-container');
	errorContainer.textContent = '';
}

async function registerUser(event) {
	event.preventDefault();
	const name = document.getElementById('name').value;
	const password = document.getElementById('password').value;
	const confirmPassword = document.getElementById('confirmPassword').value;

	if (password !== confirmPassword) {
		alert("Passwords do not match");
		return;
	}

		fetch('/api/users', {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
			},
			body: JSON.stringify({ name, password }),
		})
		.then(response => response.json())
		.then(data => {
			if (data.redirectPath) {
				window.location.href = data.redirectPath;
			} else if (data.error) {
				const errorContainer = document.getElementById('error-container');
				errorContainer.textContent = data.error;
			}
		})
		.catch(error => {
			console.error('Error:', error);
		});
}

async function login(event) {
	event.preventDefault();
	const name = document.getElementById('username').value;
	const password = document.getElementById('password').value;

	fetch('/api/login', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ name, password }),
	})
		.then(response => response.json())
		.then(data => {
			if (data.redirectPath) {
				window.location.href = data.redirectPath;
			} else if (data.error) {
				const errorContainer = document.getElementById('error-container');
				errorContainer.textContent = data.error;
			}
		})
		.catch(error => {
			console.error('Error:', error);
		});
}

function logout() {
	document.cookie.split(';').forEach(cookie => {
		const cookieName = cookie.split('=')[0].trim();
		document.cookie = `${cookieName}=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/;`;
	});

	fetch('/logout', {
		method: 'POST'
	})
		.then(response => {
			if (response.ok) {
				window.location.href = '/login';
			} else {
				window.location.href = '/';
			}
		})
		.catch(_ => {
			window.location.href = '/';
		});
}

function fetchMovies() {
	fetch('/api/movies')
		.then(response => response.json())
		.then(data => {
			displayMovies(data);
		})
		.catch(error => console.error('Error fetching movies:', error));
}

function addMovie(event) {
	event.preventDefault();
	cleanInterface();

	const imageFile = document.getElementById('movie-image').files[0];
	if (!imageFile) {
		const errorContainer = document.getElementById('error-container');
		errorContainer.textContent = 'Error failed to get image'
		return;
	}
	const reader = new FileReader();
	reader.onloadend = function () {
		let binaryData = new Uint8Array(reader.result);
		binaryData = Array.from(binaryData);

		const newMovie = {
			title: document.getElementById('movie-title').value,
			author: document.getElementById('movie-author').value,
			image: binaryData
		};


		// Make a POST request to add a new movie (adjust as needed based on your server-side implementation)
		fetch('/api/add-movie', {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
			},
			body: JSON.stringify(newMovie),
		})
			.then(response => response.json())
			.then(data => {
				if (data.message) {
					const messageContainer = document.getElementById('message-container');
					messageContainer.textContent = data.message;
				} else if (data.error) {
					const errorContainer = document.getElementById('error-container');
					errorContainer.textContent = data.error;
				}
			})
			.catch(error => {console.error('Error adding movie:', error)});
	};
	reader.readAsArrayBuffer(imageFile);

}

function previewImage(input) {
	let preview = document.getElementById('preview-image');
	let file = input.files[0];
	let reader = new FileReader();

	reader.onloadend = function () {
		preview.src = reader.result;
	}

	if (file) {
		reader.readAsDataURL(file);
	} else {
		preview.src = "";
	}
}

function previewImageData(image_url) {
	if (image_url == "") {
		return "";
	}
	fetch(`/api/thumbnail/${image_url}`)
		.then(response => response.json())
		.then(data => {
			const uint8Array = new Uint8Array(data);
			const blob = new Blob([uint8Array], { type: 'image/png' });
			const file = new File([blob], image_url, { type: 'image/png' });
			const preview = document.getElementById('preview-image');
			const reader = new FileReader();
			reader.onloadend = function () {
				preview.src = reader.result;
				return reader.result
			}
			if (file) {
				reader.readAsDataURL(file);
			} else {
				return "";
			}
		})
		.catch(error => {
			console.error('Error fetching movies:', error)
			return "";
		});
}

function displayMovies(movies) {
	const moviesList = document.getElementById('movies-list');

	// Helper function to create a Promise for image preview data
	function getPreviewImageData(image_url) {
		if (image_url == "") {
			return Promise.resolve(""); // Resolve immediately for empty image URL
		}

		return fetch(`/api/thumbnail/${image_url}`)
			.then(response => response.json())
			.then(data => {
				const uint8Array = new Uint8Array(data);
				const blob = new Blob([uint8Array], { type: 'image/png' });
				const file = new File([blob], image_url, { type: 'image/png' });

				return new Promise(resolve => {
					const reader = new FileReader();
					reader.onloadend = function () {
						resolve(reader.result);
					};

					if (file) {
						reader.readAsDataURL(file);
					} else {
						resolve("");
					}
				});
			})
			.catch(error => {
				console.error('Error fetching movies:', error);
				return Promise.resolve("");
			});
	}

	// Iterate through movies and display them
	movies.forEach(async movie => {
		const imagePreviewData = await getPreviewImageData(movie.image_url);

		moviesList.innerHTML += `
			<div>
				<strong>Title:</strong> ${movie.title} <br>
				<strong>Author:</strong> ${movie.author} <br>
				<img class="preview-image" alt="Preview Image" src="${imagePreviewData}"> <br>
				<strong>Average Rating:</strong> ${movie.avg_rating} <br>
				<strong>Number of Ratings:</strong> ${movie.num_ratings} <br>
				<form id="ratingForm_${movie.id}">
					<label for="rating_${movie.id}">Rate this movie:</label>
					<input type="number" name="rating" id="rating_${movie.id}" min="1" max="5">
					<button type="button" onclick="submitRating(event, '${movie.id}')">Submit Rating</button>
				</form>
				<hr>
			</div>`;
	});
}

function submitRating(event, movieId) {
	event.preventDefault();

	const formId = `ratingForm_${movieId}`;
	console.log(formId);
	const ratingForm = document.getElementById(formId);
	const ratingInput = ratingForm.querySelector(`#rating_${movieId}`);

	const rating = ratingInput.value;

	console.log(`Movie Id: ${movieId}, Rating: ${rating}`);
}

function fetchMyMovies() {
	const username = getUsernameFromCookie();
	fetch(`/api/movies/${username}`)
		.then(response => response.json())
		.then(data => {
			displayMyMovies(data);
		})
		.catch(error => console.error('Error fetching movies:', error));
}

function displayMyMovies(movies) {
	const moviesList = document.getElementById('movies-list');

	// Helper function to create a Promise for image preview data
	function getPreviewImageData(image_url) {
		if (image_url == "") {
			return Promise.resolve(""); // Resolve immediately for empty image URL
		}

		return fetch(`/api/thumbnail/${image_url}`)
			.then(response => response.json())
			.then(data => {
				const uint8Array = new Uint8Array(data);
				const blob = new Blob([uint8Array], { type: 'image/png' });
				const file = new File([blob], image_url, { type: 'image/png' });

				return new Promise(resolve => {
					const reader = new FileReader();
					reader.onloadend = function () {
						resolve(reader.result);
					};

					if (file) {
						reader.readAsDataURL(file);
					} else {
						resolve("");
					}
				});
			})
			.catch(error => {
				console.error('Error fetching movies:', error);
				return Promise.resolve("");
			});
	}

	// Iterate through movies and display them
	movies.forEach(async movie => {
		const imagePreviewData = await getPreviewImageData(movie.image_url);

		moviesList.innerHTML += `
			<div>
				<strong>Title:</strong> ${movie.title} <br>
				<strong>Author:</strong> ${movie.author} <br>
				<img class="preview-image" alt="Preview Image" src="${imagePreviewData}"> <br>
				<strong>Average Rating:</strong> ${movie.avg_rating} <br>
				<strong>Number of Ratings:</strong> ${movie.num_ratings} <br>
				<form id="deleteForm_${movie.id}">
					<button type="button" onclick="deleteMovie(event, '${movie.id}')">Delete movie</button>
				</form>
				<hr>
			</div>`;
	});
}

function deleteMovie(event, movieId) {
	event.preventDefault();

	console.log(`Movie Id: ${movieId} will be deleted`);
}

