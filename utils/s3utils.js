const jszip = require('jszip');
const fs = require('fs');
const path = require('path');
const archiver = require('archiver');
const { hashElement } = require('folder-hash');
const devcadeS3 = require('./devcadeS3Actions');

/**
 * Assumes file being unzip is stored in ~/uploads and
 * unzips file to its respective directory in ~/uploads
 * 
 * Returns:
 *  - name of game directory if validation/unzipping was successful
 *  - false if validation failed or unzipping was unsuccessful
 */
const unzipFile = async (file_uuid) => {
    const fileContent = fs.readFileSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}.zip`);
    const jszipInstance = new jszip();
    const result = await jszipInstance.loadAsync(fileContent);

    console.log(result.files);

    // validate zip contents
    const verifInfoArr = [...(new Set(Object.values(result.files).map(file => file.name.split('/')[0])))];
    
    const supportedImgTypes = ['png', 'jpg', 'jpeg'];
    const data = {
        icon: verifInfoArr.find(key => key.includes('icon')),
        banner: verifInfoArr.find(key => key.includes('banner')),
        gameDir: verifInfoArr.find(key => !key.includes('icon') && !key.includes('banner'))
    }
    // check to make sure no required files are missing and that an icon.<imgtype>, banner.<imgtype> and <gamedir> exist
    if (verifInfoArr.length !== 3 || !data.icon || !data.banner || !data.gameDir ||
        !supportedImgTypes.includes(data.icon.split('.')[1]) || !supportedImgTypes.includes(data.banner.split('.')[1]) ||
        data.gameDir.includes('.')) {
        // zip file may have been missing content
        return false;
    }

    const keys = Object.keys(result.files);

    try {
        if (fs.existsSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`)) {
            // File has already been unzipped
            return false;
        }
        // create the file to place items in
        fs.mkdirSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`);

        // create files and put them into the directory
        for (let key of keys) {
            const item = result.files[key];
            if (item.dir) {
                fs.mkdirSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}/${item.name}`);
            } else {
                const fileDirPath = item.name.replace('\\', '/').split('/').slice(0, -1).join('/');
                const basename = path.basename(item.name);
                if (!fs.existsSync(fileDirPath)) {
                    fs.mkdirSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}/${fileDirPath}`, { recursive: true });
                }
                fs.writeFileSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}/${fileDirPath}/${basename}`, Buffer.from(await item.async('arraybuffer')));
            }
        }
        // return validated files
        return data;
    } catch (err) {
        // failed to create temporary files somewhere
        console.log(err);
        return false;
    }

}

/**
 * Helper function that hecks to see that zip file 
 * contains 3 files:
 *  - A folder (assumed to contain game data)
 *  - icon.png/jpg (an icon image for the game)
 *  - banner.png/jpg (a banner image for the game)
 * 
 * Returns:
 *  - true if soft validation succeeded
 *  - false if soft validation failed
 */
const verifyZipContent = async (zipContent, gameDir) => {
    // get file keys
    const keys = Object.keys(zipContent.files).filter(key => {
        const keyNameArr = key.split('/');
        return (zipContent.files[key].dir && keyNameArr[0] === gameDir) ||
            (!zipContent.files[key].dir && (keyNameArr[0].includes('icon') || keyNameArr[0].includes('banner')));
    });

    const nonGameFileCount = (keys.filter(key => {
        const keyNameArr = key.split('/');
        return keyNameArr[0] !== gameDir;
    })).length;



    // make sure num files is 3
    if (nonGameFileCount !== 2) {
        // missing required files
        return false;
    }

    // make sure icon.png or icon.jpg exists
    const media = keys.filter(key => {
        const splitKey = path.basename(key).split('.');
        return !zipContent.files[key].dir && 
            (splitKey[0] === 'icon' || splitKey[0] === 'banner') &&
            (splitKey[1] === 'png' || splitKey[1] === 'jpg' || splitKey[1] === 'jpeg');
    });

    const invalidMedia = media.find(key => {
        const splitKey = path.basename(key).split('.');
        return splitKey[0] !== 'icon' && 
            splitKey[0] !== 'banner' &&
            splitKey[1] !== 'png' &&
            splitKey[1] !== 'jpg' &&
            splitKey[1] !== 'jpeg';
    });

    if (invalidMedia) {
        // some file aside from the required files exists
        return false;
    }

    // return the name of the game directory
    return {
        gameDir: keys.find(key => zipContent.files[key].dir),
        iconFilename: media.find(key => key.split('.')[0] === 'icon'),
        bannerFilename: media.find(key => key.split('.')[0] === 'banner')
    };
}

/**
 * Hashes game files and returns a MD5 hash of
 * the game files used for versioning
 * 
 * Returns:
 *  - MD5 hash
 *  - false if hashing failed
 */
const hashGameFiles = async (file_uuid, gameDirName) => {
    // TODO: remove '*' -- this is here for testing purposes
    const options = {
        files: { include: ['*', '*.cs', '*.json'] }
    }

    try {
        return await hashElement(`${devcadeS3.UPLOADS_DIR}/${file_uuid}/${gameDirName}`, options);
    } catch (err) {
        console.log(err);
        return false;
    }
}

/**
 * Zips game files only back into {file_uuid}.zip and uploads
 * this zip, along with the banner and icon files
 * 
 * Returns:
 *  - game file hash if successful
 *  - false if an issue occurred
 */
const zipGameFilesAndUpload = async (file_uuid, zipContentFiles) => {
    try {
        // hash game files
        const gameFileHash = await hashGameFiles(file_uuid, zipContentFiles.gameDir);
        if (!gameFileHash) {
            // failed to hash game file
            return false;
        }
        
        // zip game files
        await zipDirectory(
            `${devcadeS3.UPLOADS_DIR}/${file_uuid}/${zipContentFiles.gameDir}`, 
            `${devcadeS3.UPLOADS_DIR}/${file_uuid}/${file_uuid}.zip`
        );

        // get proper files for uploading to s3
        const s3UploadFiles = [
            `${file_uuid}.zip`, 
            ...Object.values(zipContentFiles)
                .filter(fname => fname !== zipContentFiles.gameDir)
            ];

        s3UploadFiles.forEach(item => console.log(item));

        // upload files to s3 bucket
        const s3Promise = new Promise((resolve, reject) => {
            try {
                var failed = false;
                s3UploadFiles.forEach(async file => {
                    const res = await devcadeS3.uploadGameFile(file_uuid, file);
                    if (!res) {
                        failed = true;
                    }
                });
                if (failed) {
                    reject('failed to upload game files to s3');
                } else {
                    resolve('success');
                }
            } catch (err) {
                reject(err);
            }
        });

        const s3Res = await s3Promise;
        if (s3Res !== 'success') {
            throw s3Res;
        }
        return gameFileHash
    } catch(err) {
        console.log(err);
        return false;
    }
}

/**
 * @param {String} sourceDir: /some/folder/to/compress
 * @param {String} outPath: /path/to/created.zip
 * @returns {Promise}
 */
const zipDirectory = (sourceDir, outPath) => {
    const archive = archiver('zip', { zlib: { level: 9 }});
    const stream = fs.createWriteStream(outPath);

    return new Promise((resolve, reject) => {
        archive
            .directory(sourceDir, false)
            .on('error', err => reject(err))
            .pipe(stream);

        stream.on('close', () => resolve());
        archive.finalize();
    });
}

/**
 * Downloads a game zip locally to ~/downloads
 * @param {string} file_uuid 
 * @returns true if success, false if failed
 */
const downloadZip = async (file_uuid) => {
    try {
        // download game zip from s3 bucket
        const s3Promise = new Promise(async (resolve, reject) => {
            try {
                const res = await devcadeS3.downloadGame(file_uuid);
                if (res) {
                    resolve('success');
                } else {
                    reject('failed to download game zip');
                }
            } catch (err) {
                reject(err);
            }
        });

        const s3Res = await s3Promise;
        if (s3Res !== 'success') {
            throw s3Res;
        }

        await devcadeS3.waitForGame(file_uuid);

        return true;
    } catch (err) {
        console.log(err);
        return false;
    }
}

/**
 * Downloads the media files for a game locally to ~/downloads
 * @param {string} file_uuid 
 * @returns true if zip was successful, false otherwise
 */
const downloadAndZipMedias = async (file_uuid) => {
    try {
        
        // download game icon and banner from s3 bucket
        const s3Promise = new Promise(async (resolve, reject) => {
            try {
                const res = await devcadeS3.downloadMedias(file_uuid);
                if (res) {
                    resolve('success');
                } else {
                    reject('failed to download medias zip');
                }
            } catch (err) {
                reject(err);
            }
        });

        const s3Res = await s3Promise;
        console.log(`S3RES: ${s3Res}`);
        if (s3Res !== 'success') {
            throw s3Res;
        }

        await devcadeS3.waitForBanner(file_uuid);
        await devcadeS3.waitForIcon(file_uuid);
        
        // zip medias
        await zipDirectory(
            `${devcadeS3.DOWNLOADS_DIR}/${file_uuid}/medias`, 
            `${devcadeS3.DOWNLOADS_DIR}/${file_uuid}/medias.zip`
        );
        return true;
    } catch (err) {
        console.log(err);
        return false;
    }
}

const downloadIcon = async (file_uuid) => {
    try {
        // download game zip from s3 bucket
        const s3Promise = new Promise(async (resolve, reject) => {
            try {
                const res = await devcadeS3.downloadIcon(file_uuid);
                if (res) {
                    resolve('success');
                } else {
                    reject('failed to download icon');
                }
            } catch (err) {
                reject(err);
            }
        });
        const s3Res = await s3Promise;
        if (s3Res !== 'success') {
            throw s3Res;
        }

        await devcadeS3.waitForIcon(file_uuid);

        return true;
    } catch (err) {
        console.log(err);
        return false;
    }
};

const downloadBanner = async (file_uuid) => {
    try {
        // download game zip from s3 bucket
        const s3Promise = new Promise(async (resolve, reject) => {
            try {
                const res = await devcadeS3.downloadBanner(file_uuid);
                if (res) {
                    resolve('success');
                } else {
                    reject('failed to download banner');
                }
            } catch (err) {
                reject(err);
            }
        });

        const s3Res = await s3Promise;
        if (s3Res !== 'success') {
            throw s3Res;
        }

        await devcadeS3.waitForBanner(file_uuid);

        return true;
    } catch (err) {
        console.log(err);
        return false;
    }
};

/**
 * delete local files associated with the game
 * @param {String} file_uuid 
 * @returns 
 */
const deleteLocalFiles = async (file_uuid) => {
    try {
        // attempt to delete uploaded zip since it isn't needed anymore
        if (fs.existsSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}.zip`)) {
            fs.unlinkSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}.zip`);
            console.log(`deleted: ${devcadeS3.UPLOADS_DIR}/${file_uuid}.zip`);
        }
        // attempt to delete unzipped files since they aren't needed anymore 
        if (fs.existsSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`)) {
            await (async () => {
                fs.rmdir(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`, { recursive: true, force: true }, async (err) => {
                    console.log("waiting for subdirectories to be deleted...")
                    
                    if (fs.existsSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`)) {
                        while (fs.readdirSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`).length !== 0) {
                            await devcadeS3.delay(250);
                            if (!fs.existsSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`)) {
                                break;
                            }
                        }
                    }
                    if (fs.existsSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`)) {
                        fs.rmdirSync(`${devcadeS3.UPLOADS_DIR}/${file_uuid}`);
                    }
                    console.log(`deleted: ${devcadeS3.UPLOADS_DIR}/${file_uuid}`);
                });
            })();
        }
        // attempt to delete download files
        if (fs.existsSync(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`)) {
            await (async () => {
                fs.rmdir(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`, { recursive: true, force: true }, async (err) => {
                    console.log("waiting for subdirectories to be deleted...")
                    
                    if (fs.existsSync(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`)) {
                        while (fs.readdirSync(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`).length !== 0) {
                            await devcadeS3.delay(250);
                            if (!fs.existsSync(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`)) {
                                break;
                            }
                        }
                    }
                    if (fs.existsSync(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`)) {
                        fs.rmdirSync(`${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`);
                    }
                    console.log(`deleted: ${devcadeS3.DOWNLOADS_DIR}/${file_uuid}`);
                });
            })();
        }
    } catch (err) {
        console.log(`Error occurred while deleting local files for: ${file_uuid}`);
        return false;
    }
}

const getBannerLocalPath = async (file_uuid) => {
    return await devcadeS3.getBannerLocalPath(file_uuid);
};

const getIconLocalPath = async (file_uuid) => {
    return await devcadeS3.getIconLocalPath(file_uuid);
};

module.exports = {
    unzipFile,
    hashGameFiles,
    zipGameFilesAndUpload,
    deleteLocalFiles,
    downloadZip,
    downloadAndZipMedias,
    getBannerLocalPath,
    getIconLocalPath,
    downloadIcon,
    downloadBanner,
    DOWNLOADS_DIR: devcadeS3.DOWNLOADS_DIR,
    UPLOADS_DIR: devcadeS3.UPLOADS_DIR
};