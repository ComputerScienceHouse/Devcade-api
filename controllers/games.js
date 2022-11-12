const os = require('os');
const fs = require('fs');
const path = require('path');
const config = require('../utils/config');
const logger = require('../utils/logger');
const db = require('../utils/database');
const s3utils = require('../utils/s3utils');
const gamesRouter = require('express').Router();
const Game = require('../models/game');
const mime = require('mime-types');

// utilities
const multer = require('multer');
const { v4: uuidv4 } = require('uuid');
const { response } = require('express');

const storage = multer.diskStorage({
    destination: (req, file, cb) => {
        cb(null, `${process.cwd()}/uploads`)
    },
    filename: (req, file, cb) => {
        cb(null, `${uuidv4()}.zip`);
    }
});
const upload = multer({
    storage: storage,
    fileFilter: (req, file, cb) => {
        if (file.mimetype !== "application/zip") {
            req.fileValidationError = "Only .zip files are allowed!";
            return cb(null, false)
        }
        cb(null, true);
    }    
});


/**************** POST ROUTES ****************/

/**
 * Attempts to upload a game to the s3 bucket and record its record in the
 * devcade postgres DB
 */
gamesRouter.post('/upload', upload.single('file'), async (req, res) => {
    if (req.fileValidationError) {
        // Received an invalid file type (only accept zips)
        res.status(400).send("Only .zip files are allowed!");
    } else {
        // The actual file
        const file = req.file;
        // The name of the game
        const game_name = req.body.title;

        console.log(game_name);
        console.log(file);

        const file_uuid = file.filename.split('.')[0];


        // unzip file
        // check zip contains 3 files: one dir, two files (icon.png/jpg and banner.png/jpg)
        const zipContent = await s3utils.unzipFile(file_uuid)
        
        if (!zipContent) {
            // failed to unzip or verify zip file content
            s3utils.deleteLocalFiles(file_uuid);
            return res.status(500).send("Upload game failed! Try again later.");
        }
        
        // hash and zip game files, then upload each file to the s3 bucket individually
        const gameFileHash = await s3utils.zipGameFilesAndUpload(file_uuid, zipContent);

        console.log(`uploadStatus: ${(gameFileHash ? 'success' : 'failed')}`);

        if (!gameFileHash) {
            // failed to hash, zip or upload game files to s3 bucket
            s3utils.deleteLocalFiles(file_uuid);
            return res.status(500).send("Upload game failed! Try again later.");
        }
        
        // if all of this succeeded, record game (title, s3 uuid, author name, game files hash value)        
        const query = 
            "INSERT INTO " + 
            "game(game_id, author_username, upload_date, game_name, hash) " +
            `VALUES ('${file.filename.split('.')[0]}', 'PLACEHOLDER_AUTHOR', NOW(), '${game_name}', '${gameFileHash.hash}');`;
        console.log(query);

        // Success, so create game record in the database
        try {
            db.pool.connect((err, client, release) => {
                if (err) {
                    console.error('Error acquiring client', err.stack);
                }

                client.query(query, (err, result) => {
                    release();
                    if (err) {
                        console.error('Error executing query', err.stack);
                    }
                });
            });
        } catch (err) {
            console.log(err);
            s3utils.deleteLocalFiles(file_uuid);
            return res.status(500).send("Upload game failed! Try again later.");
        }

        // delete all files related to the game from the server since they are stored in s3
        const delRes = s3utils.deleteLocalFiles(file_uuid);

        // game file successfully uploaded
        return res.sendStatus(200);
    }
});

/**************** GET ROUTES ****************/

gamesRouter.get('/download/:gameId', async (req, res) => {
    const gameId = req.params.gameId;
    try {
        if (!(await s3utils.downloadZip(gameId))) {
            return res.status(500).send("Failed to fetch game zip from s3 bucket");
        }

        const zipPath = `${s3utils.DOWNLOADS_DIR}/${gameId}/${gameId}.zip`;
        const destFile = path.basename(zipPath);
        const stat = fs.statSync(zipPath);

        res.writeHead(200, {
            'Content-Type': 'application/zip',
            'Content-Disposition': `attachment; filename=${destFile}`,
            'Content-Length': stat.size,
            "File-Name": path.basename(destFile)
        });

        var readStream = fs.createReadStream(zipPath);
        return readStream.pipe(res);
    } catch {
        return res.status(500).send("Failed to download game zip");
    } finally {
        // delete all files related to the game from the server since they have been sent to the user
        s3utils.deleteLocalFiles(gameId);
    }
});

gamesRouter.get('/download/medias/:gameId', async (req, res) => {
    const gameId = req.params.gameId;
    try {
        if (!(await s3utils.downloadAndZipMedias(gameId))) {
            return res.status(500).send("Failed to fetch game medias from s3 bucket");
        }

        const zipPath = `${s3utils.DOWNLOADS_DIR}/${gameId}/medias.zip`;
        const destFile = `${gameId}-${path.basename(zipPath)}`;
        const stat = fs.statSync(zipPath);

        res.writeHead(200, {
            'Content-Type': 'application/zip',
            'Content-Disposition': `attachment; filename=${path.basename(destFile)}`,
            'Content-Length': stat.size,
            "File-Name": path.basename(destFile)
        });

        var readStream = fs.createReadStream(zipPath);
        return readStream.pipe(res);
    } catch (e) {
        return res.status(500).send("Failed to download game medias zip");
    } finally {
        // delete all files related to the game from the server since they have been sent to the user
        s3utils.deleteLocalFiles(gameId);
    }
});

gamesRouter.get('/download/icon/:gameId', async (req, res) => {
    const gameId = req.params.gameId;
    try {
        if (!(await s3utils.downloadIcon(gameId))) {
            return res.status(500).send("Failed to fetch game medias from s3 bucket");
        }
        console.log("PASSED");

        const filePath = await s3utils.getIconLocalPath(gameId);
        const destFile = `${gameId}-${path.basename(filePath)}`;
        const stat = fs.statSync(filePath);

        res.writeHead(200, {
            'Content-Type': mime.lookup(path.basename(filePath)),
            'Content-Disposition': `attachment; filename=${path.basename(destFile)}`,
            'Content-Length': stat.size,
            'File-Name': path.basename(destFile)
        });

        var readStream = fs.createReadStream(filePath);
        return readStream.pipe(res);
    } catch (err) {
        console.log(err);
        return res.status(500).send("Failed to download icon");
    } finally {
        console.log("FUCK");
        // delete all files related to the game from the server since they have been sent to the user
        s3utils.deleteLocalFiles(gameId);
    }
});

gamesRouter.get('/download/banner/:gameId', async (req, res) => {
    const gameId = req.params.gameId;

    try {
        if (!(await s3utils.downloadBanner(gameId))) {
            return res.status(500).send("Failed to fetch game medias from s3 bucket");
        }

        const filePath = await s3utils.getBannerLocalPath(gameId);
        const destFile = `${gameId}-${path.basename(filePath)}`;
        const stat = fs.statSync(filePath);

        res.writeHead(200, {
            'Content-Type': mime.lookup(path.basename(filePath)),
            'Content-Disposition': `attachment; filename=${path.basename(destFile)}`,
            'Content-Length': stat.size,
            'File-Name': path.basename(destFile)
        });

        var readStream = fs.createReadStream(filePath);
        return readStream.pipe(res);
    } catch (err) {
        return res.status(500).send("Failed to download icon");
    } finally {
        // delete all files related to the game from the server since they have been sent to the user
        s3utils.deleteLocalFiles(gameId);
    }
});

gamesRouter.get('/gamelist/', async (req, res) => {
    const query = `SELECT * FROM game`;
    try {
        const client = await db.pool.connect();
        const games = await client.query(query);
        await client.end();
        
        return res.status(200).send(games.rows.map(game => {
            return new Game(
                game["game_id"], 
                game["author_username"],
                game["upload_date"],
                game["game_name"],
                game["hash"]);
        }));
    } catch (e) {
        return res.status(500).send("Failed to retrieve games list");
    }
});

gamesRouter.get('/gamelist/ids', async (req, res) => {
    const query = `SELECT game_id FROM game`;
    try {
        const client = await db.pool.connect();
        const games = await client.query(query);
        await client.end();
        
        return res.status(200).send(games.rows.map(game => game["game_id"]));
    } catch (e) {
        console.log(e);
        return res.status(500).send("Failed to retrieve games ids list");
    }
});

module.exports = gamesRouter;