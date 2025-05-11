function avatar(key) {
  return `
  <div style="width:40px; height: 40px; background: #${key.slice(0, 6)}; overflow:hidden; border-radius: 8px 8px;">
    <div style="width:40px; height: 20px; background: #${key.slice(6, 12)}">
    </div>  
  </div>
  `
}

(async () => {
  console.log('ready')

  let posts = []

  document.getElementById("post-button").onclick = async () => {
    await fetch("/post", {
      method: "post",
      headers: {
        'Accept': 'application/json',
        'Content-Type': 'application/json'
      },
      //make sure to serialize your JSON body
      body: JSON.stringify({
        body: document.getElementById("post-text").value
      })
    })

    window.location = window.location
  }

  const identity = await (await fetch("/id")).json()
  console.log(identity)

  const response = await fetch("/posts")
  const data = await response.json()

  for (logId of Object.keys(data)) {
    for (operation of data[logId]) {
      posts.push(operation)
    }
  }

  posts = posts.sort((a, b) => {
    if (a.timestamp < b.timestamp) {
      return -1
    }
    if (b.timestamp < a.timestamp) {
      return 1
    }
    return 0
  })

  let postsString = ''

  posts.map(p => {
    postsString += `
    <div class="post">
      <div>
        <div class="post-author">
          ${avatar(p.public_key)}
          ${p.public_key.slice(0, 9)} ${identity.public_key === p.public_key ? '(Me)' : ''}
        </div>
        <div>
          ${new Date(p.timestamp)}<br>
      </div>
     </div>
      ${p.body}
    </div>
    `
  })
  document.getElementById("post-list").innerHTML = postsString

  Array.from(document.getElementsByClassName("avatar-here")).map(e => e.innerHTML = avatar(identity.public_key))
})()